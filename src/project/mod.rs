pub mod renderer;

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::audio::SequencerState;
use crate::fx::{MasterFxState, TrackFxState};
use crate::sequencer::{Arrangement, PatternBank, PlaybackMode};
use crate::synth::{BassParams, HiHatParams, KickParams, SnareParams};

const PROJECT_VERSION: u32 = 1;

/// Serializable project data (excludes runtime-only fields like playing, current_step)
#[derive(Clone, Serialize, Deserialize)]
pub struct ProjectData {
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

impl ProjectData {
    /// Snapshot the current sequencer state into a serializable project
    pub fn from_state(state: &SequencerState) -> Self {
        Self {
            version: PROJECT_VERSION,
            bpm: state.bpm,
            kick_params: state.kick_params.clone(),
            snare_params: state.snare_params.clone(),
            hihat_params: state.hihat_params.clone(),
            bass_params: state.bass_params.clone(),
            track_volumes: state.track_volumes,
            track_pans: state.track_pans,
            track_mutes: state.track_mutes,
            track_solos: state.track_solos,
            track_fx: state.track_fx.clone(),
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
        SequencerState {
            playing: false,
            bpm: self.bpm,
            current_step: 0,
            pattern,
            kick_params: self.kick_params.clone(),
            snare_params: self.snare_params.clone(),
            hihat_params: self.hihat_params.clone(),
            bass_params: self.bass_params.clone(),
            track_volumes: self.track_volumes,
            track_pans: self.track_pans,
            track_mutes: self.track_mutes,
            track_solos: self.track_solos,
            track_fx: self.track_fx.clone(),
            master_fx: self.master_fx.clone(),
            pattern_bank: self.pattern_bank.clone(),
            current_pattern: self.current_pattern,
            playback_mode: self.playback_mode,
            arrangement: self.arrangement.clone(),
            arrangement_position: 0,
            arrangement_repeat: 0,
        }
    }
}

/// Save the current sequencer state to a .grox JSON file
pub fn save_project(state: &SequencerState, path: &Path) -> Result<()> {
    let project = ProjectData::from_state(state);
    let json = serde_json::to_string_pretty(&project)
        .context("Failed to serialize project")?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Load a project from a .grox JSON file
pub fn load_project(path: &Path) -> Result<ProjectData> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let project: ProjectData = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    if project.version > PROJECT_VERSION {
        bail!(
            "Project version {} is newer than supported version {}",
            project.version,
            PROJECT_VERSION
        );
    }
    Ok(project)
}
