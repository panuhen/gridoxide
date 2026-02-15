pub mod renderer;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audio::{SequencerState, TrackState};
use crate::fx::{MasterFxState, TrackFxState};
use crate::sequencer::{Arrangement, PatternBank, PlaybackMode};
use crate::synth::{load_wav, BassParams, HiHatParams, KickParams, SnareParams, SynthType};

const PROJECT_VERSION: u32 = 2;

/// Per-track data for v2 project files
#[derive(Clone, Serialize, Deserialize)]
pub struct TrackProjectData {
    pub synth_type: SynthType,
    pub name: String,
    pub default_note: u8,
    pub params: Value,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub fx: TrackFxState,
}

/// Serializable project data v2 (dynamic tracks)
#[derive(Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub version: u32,
    pub bpm: f32,
    pub tracks: Vec<TrackProjectData>,
    pub master_fx: MasterFxState,
    pub pattern_bank: PatternBank,
    pub current_pattern: usize,
    pub playback_mode: PlaybackMode,
    pub arrangement: Arrangement,
}

/// Sample buffer loaded for a sampler track during project load
pub struct SampleBuffer {
    pub track: usize,
    pub buffer: Vec<f32>,
    pub path: String,
}

/// v1 project data format (for migration from old .grox files)
#[derive(Clone, Serialize, Deserialize)]
struct ProjectDataV1 {
    pub version: u32,
    pub bpm: f32,
    pub kick_params: KickParams,
    pub snare_params: SnareParams,
    pub hihat_params: HiHatParams,
    pub bass_params: BassParams,
    pub track_volumes: [f32; 4],
    pub track_pans: [f32; 4],
    pub track_mutes: [bool; 4],
    pub track_solos: [bool; 4],
    pub track_fx: [TrackFxState; 4],
    pub master_fx: MasterFxState,
    pub pattern_bank: PatternBank,
    pub current_pattern: usize,
    pub playback_mode: PlaybackMode,
    pub arrangement: Arrangement,
}

impl ProjectDataV1 {
    fn migrate(self) -> ProjectData {
        let v1_synths = [
            (SynthType::Kick, "KICK", 36u8, serde_json::to_value(&self.kick_params).unwrap_or(Value::Null)),
            (SynthType::Snare, "SNARE", 50u8, serde_json::to_value(&self.snare_params).unwrap_or(Value::Null)),
            (SynthType::HiHat, "HIHAT", 60u8, serde_json::to_value(&self.hihat_params).unwrap_or(Value::Null)),
            (SynthType::Bass, "BASS", 33u8, serde_json::to_value(&self.bass_params).unwrap_or(Value::Null)),
        ];

        let tracks: Vec<TrackProjectData> = v1_synths
            .iter()
            .enumerate()
            .map(|(i, (synth_type, name, default_note, params))| TrackProjectData {
                synth_type: *synth_type,
                name: name.to_string(),
                default_note: *default_note,
                params: params.clone(),
                volume: self.track_volumes[i],
                pan: self.track_pans[i],
                mute: self.track_mutes[i],
                solo: self.track_solos[i],
                fx: self.track_fx[i].clone(),
            })
            .collect();

        ProjectData {
            version: PROJECT_VERSION,
            bpm: self.bpm,
            tracks,
            master_fx: self.master_fx,
            pattern_bank: self.pattern_bank,
            current_pattern: self.current_pattern,
            playback_mode: self.playback_mode,
            arrangement: self.arrangement,
        }
    }
}

impl ProjectData {
    /// Snapshot the current sequencer state into a serializable project
    pub fn from_state(state: &SequencerState) -> Self {
        let tracks: Vec<TrackProjectData> = state
            .tracks
            .iter()
            .map(|t| TrackProjectData {
                synth_type: t.synth_type,
                name: t.name.clone(),
                default_note: t.default_note,
                params: t.params_snapshot.clone(),
                volume: t.volume,
                pan: t.pan,
                mute: t.mute,
                solo: t.solo,
                fx: t.fx.clone(),
            })
            .collect();

        Self {
            version: PROJECT_VERSION,
            bpm: state.bpm,
            tracks,
            master_fx: state.master_fx.clone(),
            pattern_bank: state.pattern_bank.clone(),
            current_pattern: state.current_pattern,
            playback_mode: state.playback_mode,
            arrangement: state.arrangement.clone(),
        }
    }

    /// Reconstruct a SequencerState from project data (runtime fields default)
    pub fn to_state(&self) -> SequencerState {
        let pattern = self.pattern_bank.get(self.current_pattern).clone();
        let tracks: Vec<TrackState> = self
            .tracks
            .iter()
            .map(|t| TrackState {
                synth_type: t.synth_type,
                name: t.name.clone(),
                default_note: t.default_note,
                params_snapshot: t.params.clone(),
                volume: t.volume,
                pan: t.pan,
                mute: t.mute,
                solo: t.solo,
                fx: t.fx.clone(),
            })
            .collect();

        SequencerState {
            playing: false,
            bpm: self.bpm,
            current_step: 0,
            pattern,
            tracks,
            master_fx: self.master_fx.clone(),
            pattern_bank: self.pattern_bank.clone(),
            current_pattern: self.current_pattern,
            playback_mode: self.playback_mode,
            arrangement: self.arrangement.clone(),
            arrangement_position: 0,
            arrangement_repeat: 0,
        }
    }

    /// Convert absolute wav_path fields to relative paths (relative to project dir)
    fn make_paths_relative(&mut self, project_dir: &Path) {
        for track in &mut self.tracks {
            if track.synth_type == SynthType::Sampler {
                if let Some(wav_path) = track.params.get("wav_path").and_then(|v| v.as_str()) {
                    let abs = PathBuf::from(wav_path);
                    if abs.is_absolute() {
                        // Try to make relative to project dir
                        if let Ok(rel) = abs.strip_prefix(project_dir) {
                            track.params["wav_path"] =
                                Value::String(rel.to_string_lossy().to_string());
                        }
                        // Otherwise keep as-is (might be in global samples dir)
                    }
                }
            }
        }
    }

    /// Load WAV buffers for all sampler tracks, resolving relative paths against project dir
    pub fn load_sample_buffers(&self, project_dir: &Path) -> Vec<SampleBuffer> {
        let mut buffers = Vec::new();
        for (i, track) in self.tracks.iter().enumerate() {
            if track.synth_type != SynthType::Sampler {
                continue;
            }
            let wav_path = match track.params.get("wav_path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            // Resolve path: try relative to project dir first, then absolute, then sample dirs
            let resolved = resolve_wav_path(wav_path, project_dir);
            if let Some(full_path) = resolved {
                match load_wav(&full_path, 44100.0) {
                    Ok(buffer) => {
                        buffers.push(SampleBuffer {
                            track: i,
                            buffer,
                            path: full_path.to_string_lossy().to_string(),
                        });
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to load sample for track {}: {} ({})",
                            i, wav_path, e
                        );
                    }
                }
            } else {
                eprintln!(
                    "Warning: Sample not found for track {}: {}",
                    i, wav_path
                );
            }
        }
        buffers
    }
}

/// Resolve a wav path from a project file
fn resolve_wav_path(wav_path: &str, project_dir: &Path) -> Option<PathBuf> {
    let as_path = PathBuf::from(wav_path);

    // If absolute and exists, use it
    if as_path.is_absolute() && as_path.exists() {
        return Some(as_path);
    }

    // Try relative to project directory
    let relative_to_project = project_dir.join(wav_path);
    if relative_to_project.exists() {
        return Some(relative_to_project);
    }

    // Try sample directories
    let dirs = crate::samples::search_dirs();
    crate::samples::resolve_sample_path(wav_path, &dirs)
}

/// Save the current sequencer state to a .grox JSON file
pub fn save_project(state: &SequencerState, path: &Path) -> Result<()> {
    let mut project = ProjectData::from_state(state);
    // Convert absolute WAV paths to relative
    if let Some(project_dir) = path.parent() {
        let abs_dir = std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
        project.make_paths_relative(&abs_dir);
    }
    let json = serde_json::to_string_pretty(&project)
        .context("Failed to serialize project")?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Load a project from a .grox JSON file (supports v1 migration)
pub fn load_project(path: &Path) -> Result<ProjectData> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // Peek at version to determine format
    let raw: Value = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    let version = raw.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    if version > PROJECT_VERSION {
        bail!(
            "Project version {} is newer than supported version {}",
            version,
            PROJECT_VERSION
        );
    }

    if version <= 1 {
        // v1 format: migrate to v2
        let v1: ProjectDataV1 = serde_json::from_value(raw)
            .with_context(|| format!("Failed to parse v1 project {}", path.display()))?;
        Ok(v1.migrate())
    } else {
        // v2 format
        let project: ProjectData = serde_json::from_value(raw)
            .with_context(|| format!("Failed to parse v2 project {}", path.display()))?;
        Ok(project)
    }
}
