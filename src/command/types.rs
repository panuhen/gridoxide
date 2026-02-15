use serde::{Deserialize, Serialize};

use crate::audio::SequencerState;
use crate::fx::{FilterType, FxParamId, FxType, MasterFxParamId};
use crate::sequencer::PlaybackMode;
use crate::synth::SynthType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandSource {
    Tui,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    // Transport
    Play,
    Pause,
    Stop,
    SetBpm(f32),

    // Pattern
    ToggleStep { track: usize, step: usize },
    ClearTrack(usize),
    FillTrack(usize),

    // Per-step note
    SetStepNote { track: usize, step: usize, note: u8 },

    // Dynamic track parameter (replaces old SetKickParams/SetSnareParams/etc.)
    SetTrackParam { track: usize, key: String, value: f32 },

    // Dynamic track management
    AddTrack { synth_type: SynthType, name: String },
    RemoveTrack(usize),

    // Mixer
    SetTrackVolume { track: usize, volume: f32 },
    SetTrackPan { track: usize, pan: f32 },
    ToggleMute(usize),
    ToggleSolo(usize),

    // Per-track FX
    SetFxParam { track: usize, param: FxParamId, value: f32 },
    SetFxFilterType { track: usize, filter_type: FilterType },
    ToggleFxEnabled { track: usize, fx: FxType },

    // Master FX
    SetMasterFxParam { param: MasterFxParamId, value: f32 },
    ToggleMasterFxEnabled,

    // Pattern Bank
    SelectPattern(usize),
    CopyPattern { src: usize, dst: usize },
    ClearPattern(usize),

    // Playback Mode
    SetPlaybackMode(PlaybackMode),

    // Arrangement
    AppendArrangement { pattern: usize, repeats: usize },
    InsertArrangement { position: usize, pattern: usize, repeats: usize },
    RemoveArrangement(usize),
    SetArrangementEntry { position: usize, pattern: usize, repeats: usize },
    ClearArrangement,

    // Project I/O
    #[serde(skip)]
    LoadProject(Box<SequencerState>),

    // Sample loading
    #[serde(skip)]
    LoadSample { track: usize, buffer: Vec<f32>, path: String },
    #[serde(skip)]
    PreviewSample(Vec<f32>),
}

impl Command {
    /// Returns true if this command should be logged to event log
    pub fn is_loggable(&self) -> bool {
        !matches!(
            self,
            Command::LoadProject(_) | Command::LoadSample { .. } | Command::PreviewSample(_)
        )
    }

    /// Human-readable description of the command
    pub fn description(&self) -> String {
        match self {
            Command::Play => "Play".to_string(),
            Command::Pause => "Pause".to_string(),
            Command::Stop => "Stop".to_string(),
            Command::SetBpm(bpm) => format!("Set BPM to {}", bpm),
            Command::ToggleStep { track, step } => {
                format!("Toggle track {} step {}", track, step)
            }
            Command::ClearTrack(track) => format!("Clear track {}", track),
            Command::FillTrack(track) => format!("Fill track {}", track),
            Command::SetStepNote { track, step, note } => {
                format!("Set track {} step {} note to {}", track, step, note)
            }
            Command::SetTrackParam { track, key, value } => {
                format!("Set track {} param {} to {:.2}", track, key, value)
            }
            Command::AddTrack { synth_type, name } => {
                format!("Add {} track '{}'", synth_type.name(), name)
            }
            Command::RemoveTrack(track) => format!("Remove track {}", track),
            Command::SetTrackVolume { track, volume } => {
                format!("Set track {} volume to {:.2}", track, volume)
            }
            Command::SetTrackPan { track, pan } => {
                format!("Set track {} pan to {:.2}", track, pan)
            }
            Command::ToggleMute(track) => format!("Toggle mute track {}", track),
            Command::ToggleSolo(track) => format!("Toggle solo track {}", track),
            Command::SetFxParam { track, param, value } => {
                format!("Set track {} FX {} to {:.2}", track, param.name(), value)
            }
            Command::SetFxFilterType { track, filter_type } => {
                format!("Set track {} filter type to {}", track, filter_type.name())
            }
            Command::ToggleFxEnabled { track, fx } => {
                format!("Toggle {} on track {}", fx.name(), track)
            }
            Command::SetMasterFxParam { param, value } => {
                format!("Set master {} to {:.2}", param.name(), value)
            }
            Command::ToggleMasterFxEnabled => "Toggle master reverb".to_string(),
            Command::SelectPattern(p) => format!("Select pattern {:02}", p),
            Command::CopyPattern { src, dst } => {
                format!("Copy pattern {:02} to {:02}", src, dst)
            }
            Command::ClearPattern(p) => format!("Clear pattern {:02}", p),
            Command::SetPlaybackMode(mode) => {
                let name = match mode {
                    PlaybackMode::Pattern => "Pattern",
                    PlaybackMode::Song => "Song",
                };
                format!("Set playback mode to {}", name)
            }
            Command::AppendArrangement { pattern, repeats } => {
                format!("Append pattern {:02} x{} to arrangement", pattern, repeats)
            }
            Command::InsertArrangement {
                position,
                pattern,
                repeats,
            } => {
                format!(
                    "Insert pattern {:02} x{} at position {}",
                    pattern, repeats, position
                )
            }
            Command::RemoveArrangement(pos) => {
                format!("Remove arrangement entry {}", pos)
            }
            Command::SetArrangementEntry {
                position,
                pattern,
                repeats,
            } => {
                format!(
                    "Set arrangement entry {} to pattern {:02} x{}",
                    position, pattern, repeats
                )
            }
            Command::ClearArrangement => "Clear arrangement".to_string(),
            Command::LoadProject(_) => "Load project".to_string(),
            Command::LoadSample { track, ref path, .. } => {
                format!("Load sample '{}' into track {}", path, track)
            }
            Command::PreviewSample(_) => "Preview sample".to_string(),
        }
    }
}
