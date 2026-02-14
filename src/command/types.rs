use serde::{Deserialize, Serialize};

use crate::fx::{FilterType, FxParamId, FxType, MasterFxParamId};
use crate::synth::{BassParams, HiHatParams, KickParams, ParamId, SnareParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandSource {
    Tui,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    // Transport
    Play,
    Stop,
    SetBpm(f32),

    // Pattern
    ToggleStep { track: usize, step: usize },
    ClearTrack(usize),
    FillTrack(usize),

    // Track parameters
    SetKickParams(KickParams),
    SetSnareParams(SnareParams),
    SetHiHatParams(HiHatParams),
    SetBassParams(BassParams),

    // Per-step note
    SetStepNote { track: usize, step: usize, note: u8 },

    // Single parameter adjustment
    SetParam { param: ParamId, value: f32 },

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
}

impl Command {
    /// Returns true if this command should be logged to event log
    pub fn is_loggable(&self) -> bool {
        // All commands are currently loggable
        true
    }

    /// Human-readable description of the command
    pub fn description(&self) -> String {
        match self {
            Command::Play => "Play".to_string(),
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
            Command::SetKickParams(_) => "Set kick parameters".to_string(),
            Command::SetSnareParams(_) => "Set snare parameters".to_string(),
            Command::SetHiHatParams(_) => "Set hi-hat parameters".to_string(),
            Command::SetBassParams(_) => "Set bass parameters".to_string(),
            Command::SetParam { param, value } => {
                format!("Set {} to {:.2}", param.name(), value)
            }
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
        }
    }
}
