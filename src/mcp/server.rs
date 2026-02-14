use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::{json, Value};

use crate::audio::SequencerState;
use crate::command::{Command, CommandSender, CommandSource};
use crate::event::EventLog;
use crate::fx::{FilterType, FxParamId, FxType, MasterFxParamId};
use crate::sequencer::TrackType;
use crate::synth::{note_name, BassParams, HiHatParams, KickParams, ParamId, SnareParams, DEFAULT_NOTES};
use crate::ui::get_param_value;

/// MCP server handler for gridoxide
pub struct GridoxideMcp {
    command_sender: CommandSender,
    event_log: Arc<RwLock<EventLog>>,
    sequencer_state: Arc<RwLock<SequencerState>>,
}

impl GridoxideMcp {
    pub fn new(
        command_sender: CommandSender,
        event_log: Arc<RwLock<EventLog>>,
        sequencer_state: Arc<RwLock<SequencerState>>,
    ) -> Self {
        Self {
            command_sender,
            event_log,
            sequencer_state,
        }
    }

    /// Dispatch a command and log it
    fn dispatch(&self, cmd: Command) {
        self.event_log.write().log(cmd.clone(), CommandSource::Mcp);
        self.command_sender.send(cmd, CommandSource::Mcp);
    }

    // === Transport Tools ===

    /// Start playback
    pub fn play(&self) -> Value {
        self.dispatch(Command::Play);
        json!({ "status": "ok", "message": "Playback started" })
    }

    /// Stop playback and reset to step 0
    pub fn stop(&self) -> Value {
        self.dispatch(Command::Stop);
        json!({ "status": "ok", "message": "Playback stopped" })
    }

    /// Set the tempo in BPM (60-200)
    pub fn set_bpm(&self, bpm: f32) -> Value {
        let bpm = bpm.clamp(60.0, 200.0);
        self.dispatch(Command::SetBpm(bpm));
        json!({ "status": "ok", "bpm": bpm })
    }

    /// Get current transport state
    pub fn get_state(&self) -> Value {
        let state = self.sequencer_state.read();
        json!({
            "playing": state.playing,
            "bpm": state.bpm,
            "current_step": state.current_step
        })
    }

    // === Pattern Tools ===

    /// Toggle a step on/off (track: 0-3, step: 0-15), optionally setting a note
    pub fn toggle_step(&self, track: usize, step: usize, note: Option<u8>) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }
        if step >= 16 {
            return json!({ "status": "error", "message": "Step must be 0-15" });
        }

        // If a note is provided, set it before toggling
        if let Some(n) = note {
            let clamped = n.min(127);
            self.dispatch(Command::SetStepNote { track, step, note: clamped });
        }

        self.dispatch(Command::ToggleStep { track, step });

        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");

        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "step": step
        })
    }

    /// Get the full pattern grid (including note data)
    pub fn get_pattern(&self) -> Value {
        let state = self.sequencer_state.read();
        let tracks: Vec<Value> = (0..4)
            .map(|track| {
                let track_type = TrackType::from_index(track).unwrap();
                let steps: Vec<bool> = (0..16).map(|step| state.pattern.get(track, step)).collect();
                let notes: Vec<Value> = (0..16)
                    .map(|step| {
                        let sd = state.pattern.get_step(track, step);
                        json!({
                            "note": sd.note,
                            "note_name": note_name(sd.note)
                        })
                    })
                    .collect();
                json!({
                    "track": track,
                    "name": track_type.name(),
                    "steps": steps,
                    "notes": notes,
                    "default_note": DEFAULT_NOTES[track]
                })
            })
            .collect();

        json!({
            "tracks": tracks
        })
    }

    /// Set the MIDI note for a specific step
    pub fn set_step_note(&self, track: usize, step: usize, note: u8) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }
        if step >= 16 {
            return json!({ "status": "error", "message": "Step must be 0-15" });
        }
        let clamped = note.min(127);

        self.dispatch(Command::SetStepNote { track, step, note: clamped });

        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");

        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "step": step,
            "note": clamped,
            "note_name": note_name(clamped)
        })
    }

    /// Get all step data for a track including notes
    pub fn get_step_notes(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        let state = self.sequencer_state.read();
        let track_type = TrackType::from_index(track).unwrap();
        let steps: Vec<Value> = (0..16)
            .map(|step| {
                let sd = state.pattern.get_step(track, step);
                json!({
                    "step": step,
                    "active": sd.active,
                    "note": sd.note,
                    "note_name": note_name(sd.note)
                })
            })
            .collect();

        json!({
            "track": track,
            "name": track_type.name(),
            "default_note": DEFAULT_NOTES[track],
            "steps": steps
        })
    }

    /// Clear all steps on a track
    pub fn clear_track(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        self.dispatch(Command::ClearTrack(track));

        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");

        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "message": format!("Cleared {}", track_name)
        })
    }

    /// Fill all steps on a track
    pub fn fill_track(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        self.dispatch(Command::FillTrack(track));

        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");

        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "message": format!("Filled {}", track_name)
        })
    }

    // === Event Query ===

    /// Get events since a given ID (for "listening" to human actions)
    pub fn get_events(&self, since_id: u64) -> Value {
        let log = self.event_log.read();
        let events = log.get_events_since(since_id);
        json!({
            "events": events,
            "latest_id": log.latest_id()
        })
    }

    // === Track Parameter Tools ===

    /// List all tracks with their synth types and parameter names
    pub fn list_tracks(&self) -> Value {
        let tracks: Vec<Value> = (0..4)
            .map(|track| {
                let track_type = TrackType::from_index(track).unwrap();
                let params = ParamId::params_for_track(track);
                let param_names: Vec<&str> = params.iter().map(|p| p.name()).collect();
                let param_keys: Vec<&str> = params.iter().map(|p| p.key()).collect();

                json!({
                    "track": track,
                    "name": track_type.name(),
                    "params": param_keys,
                    "param_names": param_names
                })
            })
            .collect();

        json!({ "tracks": tracks })
    }

    /// Get all parameters for a specific track
    pub fn get_track_params(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        let state = self.sequencer_state.read();
        let track_type = TrackType::from_index(track).unwrap();
        let params = ParamId::params_for_track(track);

        let param_values: Vec<Value> = params
            .iter()
            .map(|p| {
                let value = get_param_value(&state, *p);
                let (min, max, default) = p.range();
                json!({
                    "key": p.key(),
                    "name": p.name(),
                    "value": value,
                    "min": min,
                    "max": max,
                    "default": default
                })
            })
            .collect();

        json!({
            "track": track,
            "name": track_type.name(),
            "params": param_values
        })
    }

    /// Set a single parameter by key
    pub fn set_param(&self, param_key: &str, value: f32) -> Value {
        let param = match ParamId::from_key(param_key) {
            Some(p) => p,
            None => {
                return json!({
                    "status": "error",
                    "message": format!("Unknown parameter: {}", param_key)
                })
            }
        };

        let (min, max, _default) = param.range();
        let clamped_value = value.clamp(min, max);

        self.dispatch(Command::SetParam {
            param,
            value: clamped_value,
        });

        json!({
            "status": "ok",
            "param": param_key,
            "name": param.name(),
            "value": clamped_value,
            "min": min,
            "max": max
        })
    }

    /// Reset a track to default parameters
    pub fn reset_track(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        let track_type = TrackType::from_index(track).unwrap();

        match track {
            0 => self.dispatch(Command::SetKickParams(KickParams::default())),
            1 => self.dispatch(Command::SetSnareParams(SnareParams::default())),
            2 => self.dispatch(Command::SetHiHatParams(HiHatParams::default())),
            3 => self.dispatch(Command::SetBassParams(BassParams::default())),
            _ => unreachable!(),
        }

        json!({
            "status": "ok",
            "track": track,
            "name": track_type.name(),
            "message": format!("Reset {} to default parameters", track_type.name())
        })
    }

    // === Mixer Tools ===

    /// Get all mixer state
    pub fn get_mixer(&self) -> Value {
        let state = self.sequencer_state.read();
        let track_names = ["kick", "snare", "hihat", "bass"];
        let tracks: Vec<Value> = (0..4)
            .map(|i| {
                json!({
                    "track": i,
                    "name": track_names[i],
                    "volume": state.track_volumes[i],
                    "pan": state.track_pans[i],
                    "mute": state.track_mutes[i],
                    "solo": state.track_solos[i]
                })
            })
            .collect();
        json!({ "tracks": tracks })
    }

    /// Set track volume (0.0-1.0)
    pub fn set_volume(&self, track: usize, volume: f32) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }
        let volume = volume.clamp(0.0, 1.0);
        self.dispatch(Command::SetTrackVolume { track, volume });
        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");
        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "volume": volume
        })
    }

    /// Set track pan (-1.0 to 1.0)
    pub fn set_pan(&self, track: usize, pan: f32) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }
        let pan = pan.clamp(-1.0, 1.0);
        self.dispatch(Command::SetTrackPan { track, pan });
        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");
        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "pan": pan
        })
    }

    /// Toggle track mute
    pub fn toggle_mute(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }
        self.dispatch(Command::ToggleMute(track));
        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");
        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "message": format!("Toggled mute on {}", track_name)
        })
    }

    /// Toggle track solo
    pub fn toggle_solo(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }
        self.dispatch(Command::ToggleSolo(track));
        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");
        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "message": format!("Toggled solo on {}", track_name)
        })
    }

    // === FX Tools ===

    /// Get all FX parameters for a track
    pub fn get_fx_params(&self, track: usize) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        let state = self.sequencer_state.read();
        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");
        let fx = &state.track_fx[track];

        json!({
            "track": track,
            "name": track_name,
            "filter": {
                "enabled": fx.filter_enabled,
                "type": fx.filter_type.name(),
                "cutoff": fx.filter_cutoff,
                "cutoff_range": [20.0, 20000.0],
                "resonance": fx.filter_resonance,
                "resonance_range": [0.0, 0.95]
            },
            "distortion": {
                "enabled": fx.dist_enabled,
                "drive": fx.dist_drive,
                "drive_range": [0.0, 1.0],
                "mix": fx.dist_mix,
                "mix_range": [0.0, 1.0]
            },
            "delay": {
                "enabled": fx.delay_enabled,
                "time": fx.delay_time,
                "time_range": [10.0, 500.0],
                "feedback": fx.delay_feedback,
                "feedback_range": [0.0, 0.9],
                "mix": fx.delay_mix,
                "mix_range": [0.0, 1.0]
            }
        })
    }

    /// Set a per-track FX parameter
    pub fn set_fx_param(&self, track: usize, param_key: &str, value: f32) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        // Handle filter type specially
        if param_key == "filter_type" {
            let ft = match value as usize {
                0 => FilterType::LowPass,
                1 => FilterType::HighPass,
                2 => FilterType::BandPass,
                _ => return json!({ "status": "error", "message": "Filter type must be 0 (LP), 1 (HP), or 2 (BP)" }),
            };
            self.dispatch(Command::SetFxFilterType { track, filter_type: ft });
            return json!({
                "status": "ok",
                "track": track,
                "param": "filter_type",
                "value": ft.name()
            });
        }

        let param = match FxParamId::from_key(param_key) {
            Some(p) => p,
            None => {
                return json!({
                    "status": "error",
                    "message": format!("Unknown FX parameter: {}. Valid: filter_cutoff, filter_resonance, filter_type, dist_drive, dist_mix, delay_time, delay_feedback, delay_mix", param_key)
                })
            }
        };

        let (min, max, _default) = param.range();
        let clamped = value.clamp(min, max);

        self.dispatch(Command::SetFxParam { track, param, value: clamped });

        json!({
            "status": "ok",
            "track": track,
            "param": param_key,
            "name": param.name(),
            "value": clamped,
            "min": min,
            "max": max
        })
    }

    /// Toggle a per-track FX on/off
    pub fn toggle_fx(&self, track: usize, fx_name: &str) -> Value {
        if track >= 4 {
            return json!({ "status": "error", "message": "Track must be 0-3" });
        }

        let fx = match fx_name {
            "filter" => FxType::Filter,
            "distortion" | "dist" => FxType::Distortion,
            "delay" => FxType::Delay,
            _ => {
                return json!({
                    "status": "error",
                    "message": format!("Unknown FX type: {}. Valid: filter, distortion, delay", fx_name)
                })
            }
        };

        self.dispatch(Command::ToggleFxEnabled { track, fx });

        let track_name = TrackType::from_index(track)
            .map(|t| t.name())
            .unwrap_or("unknown");

        json!({
            "status": "ok",
            "track": track,
            "track_name": track_name,
            "fx": fx.name(),
            "message": format!("Toggled {} on {}", fx.name(), track_name)
        })
    }

    /// Get master FX (reverb) parameters
    pub fn get_master_fx_params(&self) -> Value {
        let state = self.sequencer_state.read();
        let mfx = &state.master_fx;

        json!({
            "reverb": {
                "enabled": mfx.reverb_enabled,
                "decay": mfx.reverb_decay,
                "decay_range": [0.1, 0.95],
                "mix": mfx.reverb_mix,
                "mix_range": [0.0, 1.0],
                "damping": mfx.reverb_damping,
                "damping_range": [0.0, 1.0]
            }
        })
    }

    /// Set a master FX parameter
    pub fn set_master_fx_param(&self, param_key: &str, value: f32) -> Value {
        let param = match MasterFxParamId::from_key(param_key) {
            Some(p) => p,
            None => {
                return json!({
                    "status": "error",
                    "message": format!("Unknown master FX parameter: {}. Valid: reverb_decay, reverb_mix, reverb_damping", param_key)
                })
            }
        };

        let (min, max, _default) = param.range();
        let clamped = value.clamp(min, max);

        self.dispatch(Command::SetMasterFxParam { param, value: clamped });

        json!({
            "status": "ok",
            "param": param_key,
            "name": param.name(),
            "value": clamped,
            "min": min,
            "max": max
        })
    }

    /// Toggle master reverb on/off
    pub fn toggle_master_fx(&self) -> Value {
        self.dispatch(Command::ToggleMasterFxEnabled);
        json!({
            "status": "ok",
            "message": "Toggled master reverb"
        })
    }

    /// Handle an MCP tool call
    pub fn handle_tool_call(&self, tool: &str, args: &Value) -> Value {
        match tool {
            // Transport
            "play" => self.play(),
            "stop" => self.stop(),
            "set_bpm" => {
                let bpm = args.get("bpm").and_then(|v| v.as_f64()).unwrap_or(120.0) as f32;
                self.set_bpm(bpm)
            }
            "get_state" => self.get_state(),

            // Pattern
            "toggle_step" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let step = args.get("step").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let note = args.get("note").and_then(|v| v.as_u64()).map(|n| n as u8);
                self.toggle_step(track, step, note)
            }
            "get_pattern" => self.get_pattern(),
            "set_step_note" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let step = args.get("step").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let note = args.get("note").and_then(|v| v.as_u64()).unwrap_or(60) as u8;
                self.set_step_note(track, step, note)
            }
            "get_step_notes" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.get_step_notes(track)
            }
            "clear_track" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.clear_track(track)
            }
            "fill_track" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.fill_track(track)
            }

            // Events
            "get_events" => {
                let since_id = args.get("since_id").and_then(|v| v.as_u64()).unwrap_or(0);
                self.get_events(since_id)
            }

            // Track Parameters
            "list_tracks" => self.list_tracks(),
            "get_track_params" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.get_track_params(track)
            }
            "set_param" => {
                let param = args
                    .get("param")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                self.set_param(param, value)
            }
            "reset_track" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.reset_track(track)
            }

            // Mixer
            "get_mixer" => self.get_mixer(),
            "set_volume" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let volume = args.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.8) as f32;
                self.set_volume(track, volume)
            }
            "set_pan" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let pan = args.get("pan").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                self.set_pan(track, pan)
            }
            "toggle_mute" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.toggle_mute(track)
            }
            "toggle_solo" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.toggle_solo(track)
            }

            // FX
            "get_fx_params" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.get_fx_params(track)
            }
            "set_fx_param" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let param = args.get("param").and_then(|v| v.as_str()).unwrap_or("");
                let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                self.set_fx_param(track, param, value)
            }
            "toggle_fx" => {
                let track = args.get("track").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let fx = args.get("fx").and_then(|v| v.as_str()).unwrap_or("");
                self.toggle_fx(track, fx)
            }
            "get_master_fx_params" => self.get_master_fx_params(),
            "set_master_fx_param" => {
                let param = args.get("param").and_then(|v| v.as_str()).unwrap_or("");
                let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                self.set_master_fx_param(param, value)
            }
            "toggle_master_fx" => self.toggle_master_fx(),

            _ => json!({ "status": "error", "message": format!("Unknown tool: {}", tool) }),
        }
    }

    /// Get the list of available tools (for MCP discovery)
    pub fn list_tools() -> Value {
        json!({
            "tools": [
                {
                    "name": "play",
                    "description": "Start playback",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "stop",
                    "description": "Stop playback and reset to step 0",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "set_bpm",
                    "description": "Set the tempo in BPM (60-200)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "bpm": {
                                "type": "number",
                                "description": "Tempo in beats per minute (60-200)"
                            }
                        },
                        "required": ["bpm"]
                    }
                },
                {
                    "name": "get_state",
                    "description": "Get current transport state (playing, bpm, current_step)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "toggle_step",
                    "description": "Toggle a step on/off. Tracks: 0=kick, 1=snare, 2=hihat, 3=bass. Steps: 0-15.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            },
                            "step": {
                                "type": "integer",
                                "description": "Step index (0-15)"
                            },
                            "note": {
                                "type": "integer",
                                "description": "Optional MIDI note (0-127) to set before toggling. If omitted, uses the step's existing note."
                            }
                        },
                        "required": ["track", "step"]
                    }
                },
                {
                    "name": "get_pattern",
                    "description": "Get the full pattern grid showing all tracks and steps",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "set_step_note",
                    "description": "Set the MIDI note for a step. Each step can have its own pitch (0-127). Affects how the synth sounds when that step triggers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            },
                            "step": {
                                "type": "integer",
                                "description": "Step index (0-15)"
                            },
                            "note": {
                                "type": "integer",
                                "description": "MIDI note number (0-127). 60=C4, 69=A4(440Hz). Bass: sets frequency. Kick: scales pitch. Snare: scales tone. HiHat: scales brightness."
                            }
                        },
                        "required": ["track", "step", "note"]
                    }
                },
                {
                    "name": "get_step_notes",
                    "description": "Get all step data for a track including notes. Shows active state and MIDI note for each of the 16 steps.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "clear_track",
                    "description": "Clear all steps on a track",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "fill_track",
                    "description": "Fill all steps on a track (all 16 steps active)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "get_events",
                    "description": "Get recent events/commands since a given ID. Use this to 'listen' to what the human is doing.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "since_id": {
                                "type": "integer",
                                "description": "Return events with ID greater than this value. Use 0 to get all recent events."
                            }
                        }
                    }
                },
                {
                    "name": "list_tracks",
                    "description": "List all tracks with their synth types and available parameters",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "get_track_params",
                    "description": "Get all parameters for a specific track with current values, ranges, and defaults",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "set_param",
                    "description": "Set a synth parameter by key. Use list_tracks or get_track_params to see available parameter keys.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "param": {
                                "type": "string",
                                "description": "Parameter key (e.g., 'kick_pitch_start', 'snare_tone_freq', 'hihat_decay', 'bass_frequency')"
                            },
                            "value": {
                                "type": "number",
                                "description": "New value for the parameter (will be clamped to valid range)"
                            }
                        },
                        "required": ["param", "value"]
                    }
                },
                {
                    "name": "reset_track",
                    "description": "Reset all parameters on a track to their default values",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "get_mixer",
                    "description": "Get all mixer state (volumes, pans, mutes, solos) for all tracks",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "set_volume",
                    "description": "Set track volume (0.0-1.0)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            },
                            "volume": {
                                "type": "number",
                                "description": "Volume level (0.0 to 1.0)",
                                "minimum": 0.0,
                                "maximum": 1.0
                            }
                        },
                        "required": ["track", "volume"]
                    }
                },
                {
                    "name": "set_pan",
                    "description": "Set track pan (-1.0 left to 1.0 right, 0.0 center)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            },
                            "pan": {
                                "type": "number",
                                "description": "Pan position (-1.0 = full left, 0.0 = center, 1.0 = full right)",
                                "minimum": -1.0,
                                "maximum": 1.0
                            }
                        },
                        "required": ["track", "pan"]
                    }
                },
                {
                    "name": "toggle_mute",
                    "description": "Toggle mute on a track. Muted tracks produce no audio.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "toggle_solo",
                    "description": "Toggle solo on a track. When any track is soloed, only soloed tracks are audible.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "get_fx_params",
                    "description": "Get all FX parameters for a track (filter, distortion, delay) with current values and ranges.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            }
                        },
                        "required": ["track"]
                    }
                },
                {
                    "name": "set_fx_param",
                    "description": "Set a per-track FX parameter. Params: filter_cutoff (20-20000 Hz), filter_resonance (0-0.95), filter_type (0=LP, 1=HP, 2=BP), dist_drive (0-1), dist_mix (0-1), delay_time (10-500 ms), delay_feedback (0-0.9), delay_mix (0-1).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            },
                            "param": {
                                "type": "string",
                                "description": "Parameter key (e.g., 'filter_cutoff', 'dist_drive', 'delay_time')"
                            },
                            "value": {
                                "type": "number",
                                "description": "New value (will be clamped to valid range)"
                            }
                        },
                        "required": ["track", "param", "value"]
                    }
                },
                {
                    "name": "toggle_fx",
                    "description": "Toggle a per-track effect on/off. Each track has filter, distortion, and delay (all off by default).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "track": {
                                "type": "integer",
                                "description": "Track index (0=kick, 1=snare, 2=hihat, 3=bass)"
                            },
                            "fx": {
                                "type": "string",
                                "description": "Effect name: 'filter', 'distortion', or 'delay'"
                            }
                        },
                        "required": ["track", "fx"]
                    }
                },
                {
                    "name": "get_master_fx_params",
                    "description": "Get master bus FX parameters (reverb) with current values and ranges.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "set_master_fx_param",
                    "description": "Set a master bus FX parameter. Params: reverb_decay (0.1-0.95), reverb_mix (0-1), reverb_damping (0-1).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "param": {
                                "type": "string",
                                "description": "Parameter key: 'reverb_decay', 'reverb_mix', or 'reverb_damping'"
                            },
                            "value": {
                                "type": "number",
                                "description": "New value (will be clamped to valid range)"
                            }
                        },
                        "required": ["param", "value"]
                    }
                },
                {
                    "name": "toggle_master_fx",
                    "description": "Toggle master reverb on/off.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        })
    }
}
