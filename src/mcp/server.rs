use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::{json, Value};

use crate::audio::SequencerState;
use crate::command::{Command, CommandSender, CommandSource};
use crate::event::EventLog;
use crate::fx::{FilterType, FxParamId, FxType, MasterFxParamId};
use crate::project;
use crate::project::renderer::{ExportMode, export_wav};
use crate::sequencer::{PlaybackMode, TrackType, NUM_PATTERNS};
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

    /// Pause playback (keep position)
    pub fn pause(&self) -> Value {
        self.dispatch(Command::Pause);
        json!({ "status": "ok", "message": "Playback paused" })
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
        let mode_str = match state.playback_mode {
            PlaybackMode::Pattern => "pattern",
            PlaybackMode::Song => "song",
        };
        json!({
            "playing": state.playing,
            "bpm": state.bpm,
            "current_step": state.current_step,
            "current_pattern": state.current_pattern,
            "playback_mode": mode_str,
            "arrangement_position": state.arrangement_position,
            "arrangement_repeat": state.arrangement_repeat
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

    /// Get the full pattern grid (including note data). If pattern_index is Some, read from bank.
    pub fn get_pattern(&self, pattern_index: Option<usize>) -> Value {
        let state = self.sequencer_state.read();
        let pat = match pattern_index {
            Some(idx) if idx < NUM_PATTERNS => state.pattern_bank.get(idx),
            _ => &state.pattern,
        };
        let display_idx = pattern_index.unwrap_or(state.current_pattern);

        let tracks: Vec<Value> = (0..4)
            .map(|track| {
                let track_type = TrackType::from_index(track).unwrap();
                let steps: Vec<bool> = (0..16).map(|step| pat.get(track, step)).collect();
                let notes: Vec<Value> = (0..16)
                    .map(|step| {
                        let sd = pat.get_step(track, step);
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
            "pattern": display_idx,
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

    // === Pattern Bank Tools ===

    /// Select active pattern (0-15)
    pub fn select_pattern(&self, pattern: usize) -> Value {
        if pattern >= NUM_PATTERNS {
            return json!({ "status": "error", "message": "Pattern must be 0-15" });
        }
        self.dispatch(Command::SelectPattern(pattern));
        json!({
            "status": "ok",
            "pattern": pattern,
            "message": format!("Selected pattern {:02}", pattern)
        })
    }

    /// Get overview of all 16 patterns
    pub fn get_pattern_bank(&self) -> Value {
        let state = self.sequencer_state.read();
        let patterns: Vec<Value> = (0..NUM_PATTERNS)
            .map(|i| {
                let has_content = state.pattern_bank.has_content(i);
                let active_steps: usize = (0..4)
                    .map(|t| (0..16).filter(|&s| state.pattern_bank.get(i).get(t, s)).count())
                    .sum();
                json!({
                    "index": i,
                    "has_content": has_content,
                    "active_steps": active_steps,
                    "is_current": i == state.current_pattern
                })
            })
            .collect();

        json!({
            "current_pattern": state.current_pattern,
            "patterns": patterns
        })
    }

    /// Copy pattern from src to dst
    pub fn copy_pattern(&self, src: usize, dst: usize) -> Value {
        if src >= NUM_PATTERNS || dst >= NUM_PATTERNS {
            return json!({ "status": "error", "message": "Pattern indices must be 0-15" });
        }
        self.dispatch(Command::CopyPattern { src, dst });
        json!({
            "status": "ok",
            "message": format!("Copied pattern {:02} to {:02}", src, dst)
        })
    }

    /// Clear a pattern slot
    pub fn clear_pattern(&self, pattern: usize) -> Value {
        if pattern >= NUM_PATTERNS {
            return json!({ "status": "error", "message": "Pattern must be 0-15" });
        }
        self.dispatch(Command::ClearPattern(pattern));
        json!({
            "status": "ok",
            "message": format!("Cleared pattern {:02}", pattern)
        })
    }

    /// Set playback mode
    pub fn set_playback_mode(&self, mode: &str) -> Value {
        let playback_mode = match mode {
            "pattern" => PlaybackMode::Pattern,
            "song" => PlaybackMode::Song,
            _ => {
                return json!({
                    "status": "error",
                    "message": "Mode must be 'pattern' or 'song'"
                })
            }
        };
        self.dispatch(Command::SetPlaybackMode(playback_mode));
        json!({
            "status": "ok",
            "mode": mode,
            "message": format!("Set playback mode to {}", mode)
        })
    }

    // === Arrangement Tools ===

    /// Get full arrangement
    pub fn get_arrangement(&self) -> Value {
        let state = self.sequencer_state.read();
        let entries: Vec<Value> = state
            .arrangement
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                json!({
                    "position": i,
                    "pattern": e.pattern,
                    "repeats": e.repeats,
                    "is_playing": state.playback_mode == PlaybackMode::Song && i == state.arrangement_position
                })
            })
            .collect();

        let mode_str = match state.playback_mode {
            PlaybackMode::Pattern => "pattern",
            PlaybackMode::Song => "song",
        };

        json!({
            "entries": entries,
            "length": state.arrangement.len(),
            "playback_mode": mode_str,
            "current_position": state.arrangement_position,
            "current_repeat": state.arrangement_repeat
        })
    }

    /// Append entry to arrangement
    pub fn append_arrangement(&self, pattern: usize, repeats: usize) -> Value {
        if pattern >= NUM_PATTERNS {
            return json!({ "status": "error", "message": "Pattern must be 0-15" });
        }
        let repeats = repeats.clamp(1, 16);
        self.dispatch(Command::AppendArrangement { pattern, repeats });
        json!({
            "status": "ok",
            "message": format!("Appended pattern {:02} x{} to arrangement", pattern, repeats)
        })
    }

    /// Insert entry into arrangement
    pub fn insert_arrangement(&self, position: usize, pattern: usize, repeats: usize) -> Value {
        if pattern >= NUM_PATTERNS {
            return json!({ "status": "error", "message": "Pattern must be 0-15" });
        }
        let state = self.sequencer_state.read();
        if position > state.arrangement.len() {
            return json!({ "status": "error", "message": "Position out of range" });
        }
        drop(state);
        let repeats = repeats.clamp(1, 16);
        self.dispatch(Command::InsertArrangement {
            position,
            pattern,
            repeats,
        });
        json!({
            "status": "ok",
            "message": format!("Inserted pattern {:02} x{} at position {}", pattern, repeats, position)
        })
    }

    /// Remove entry from arrangement
    pub fn remove_arrangement(&self, position: usize) -> Value {
        let state = self.sequencer_state.read();
        if position >= state.arrangement.len() {
            return json!({ "status": "error", "message": "Position out of range" });
        }
        drop(state);
        self.dispatch(Command::RemoveArrangement(position));
        json!({
            "status": "ok",
            "message": format!("Removed arrangement entry at position {}", position)
        })
    }

    /// Modify an arrangement entry
    pub fn set_arrangement_entry(&self, position: usize, pattern: usize, repeats: usize) -> Value {
        if pattern >= NUM_PATTERNS {
            return json!({ "status": "error", "message": "Pattern must be 0-15" });
        }
        let state = self.sequencer_state.read();
        if position >= state.arrangement.len() {
            return json!({ "status": "error", "message": "Position out of range" });
        }
        drop(state);
        let repeats = repeats.clamp(1, 16);
        self.dispatch(Command::SetArrangementEntry {
            position,
            pattern,
            repeats,
        });
        json!({
            "status": "ok",
            "message": format!("Set entry {} to pattern {:02} x{}", position, pattern, repeats)
        })
    }

    /// Clear arrangement
    pub fn clear_arrangement(&self) -> Value {
        self.dispatch(Command::ClearArrangement);
        json!({
            "status": "ok",
            "message": "Cleared arrangement"
        })
    }

    // === Project I/O Tools ===

    /// Save project to a .grox JSON file
    pub fn save_project(&self, path_str: &str) -> Value {
        let path = Path::new(path_str);
        let state = self.sequencer_state.read();
        match project::save_project(&state, path) {
            Ok(()) => json!({
                "status": "ok",
                "path": path_str,
                "message": format!("Saved project to {}", path_str)
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Failed to save: {}", e)
            }),
        }
    }

    /// Load project from a .grox JSON file
    pub fn load_project(&self, path_str: &str) -> Value {
        let path = Path::new(path_str);
        match project::load_project(path) {
            Ok(project_data) => {
                let new_state = project_data.to_state();
                self.dispatch(Command::LoadProject(Box::new(new_state)));
                json!({
                    "status": "ok",
                    "path": path_str,
                    "message": format!("Loaded project from {}", path_str)
                })
            }
            Err(e) => json!({
                "status": "error",
                "message": format!("Failed to load: {}", e)
            }),
        }
    }

    /// Export audio as WAV file
    pub fn export_wav_file(&self, path_str: &str, mode: &str, pattern: Option<usize>) -> Value {
        let path = Path::new(path_str);
        let state = self.sequencer_state.read();

        let export_mode = match mode {
            "pattern" => {
                let idx = pattern.unwrap_or(state.current_pattern);
                if idx >= NUM_PATTERNS {
                    return json!({ "status": "error", "message": "Pattern index must be 0-15" });
                }
                ExportMode::Pattern(idx)
            }
            "song" => ExportMode::Song,
            _ => {
                return json!({
                    "status": "error",
                    "message": "Mode must be 'pattern' or 'song'"
                })
            }
        };

        match export_wav(&state, export_mode, path) {
            Ok(result) => json!({
                "status": "ok",
                "path": path_str,
                "duration_secs": result.duration_secs,
                "samples": result.samples,
                "message": format!("Exported {:.1}s of audio to {}", result.duration_secs, path_str)
            }),
            Err(e) => json!({
                "status": "error",
                "message": format!("Failed to export: {}", e)
            }),
        }
    }

    /// List .grox project files in a directory
    pub fn list_projects(&self, directory: Option<&str>) -> Value {
        let dir = directory.unwrap_or(".");
        let path = Path::new(dir);

        if !path.is_dir() {
            return json!({
                "status": "error",
                "message": format!("Not a directory: {}", dir)
            });
        }

        let mut files: Vec<String> = Vec::new();
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().map(|e| e == "grox").unwrap_or(false) {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            files.push(name.to_string());
                        }
                    }
                }
            }
            Err(e) => {
                return json!({
                    "status": "error",
                    "message": format!("Failed to read directory: {}", e)
                });
            }
        }

        files.sort();
        json!({
            "status": "ok",
            "directory": dir,
            "files": files,
            "count": files.len()
        })
    }

    /// Handle an MCP tool call
    pub fn handle_tool_call(&self, tool: &str, args: &Value) -> Value {
        match tool {
            // Transport
            "play" => self.play(),
            "pause" => self.pause(),
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
            "get_pattern" => {
                let pattern_index = args.get("pattern").and_then(|v| v.as_u64()).map(|n| n as usize);
                self.get_pattern(pattern_index)
            }
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

            // Pattern Bank
            "select_pattern" => {
                let pattern = args.get("pattern").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.select_pattern(pattern)
            }
            "get_pattern_bank" => self.get_pattern_bank(),
            "copy_pattern" => {
                let src = args.get("src").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let dst = args.get("dst").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.copy_pattern(src, dst)
            }
            "clear_pattern" => {
                let pattern = args.get("pattern").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.clear_pattern(pattern)
            }
            "set_playback_mode" => {
                let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("pattern");
                self.set_playback_mode(mode)
            }

            // Arrangement
            "get_arrangement" => self.get_arrangement(),
            "append_arrangement" => {
                let pattern = args.get("pattern").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let repeats = args.get("repeats").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                self.append_arrangement(pattern, repeats)
            }
            "insert_arrangement" => {
                let position = args.get("position").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let pattern = args.get("pattern").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let repeats = args.get("repeats").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                self.insert_arrangement(position, pattern, repeats)
            }
            "remove_arrangement" => {
                let position = args.get("position").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.remove_arrangement(position)
            }
            "set_arrangement_entry" => {
                let position = args.get("position").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let pattern = args.get("pattern").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let repeats = args.get("repeats").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                self.set_arrangement_entry(position, pattern, repeats)
            }
            "clear_arrangement" => self.clear_arrangement(),

            // Project I/O
            "save_project" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("project.grox");
                self.save_project(path)
            }
            "load_project" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("project.grox");
                self.load_project(path)
            }
            "export_wav" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("export.wav");
                let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("pattern");
                let pattern = args.get("pattern").and_then(|v| v.as_u64()).map(|n| n as usize);
                self.export_wav_file(path, mode, pattern)
            }
            "list_projects" => {
                let directory = args.get("directory").and_then(|v| v.as_str());
                self.list_projects(directory)
            }

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
                    "name": "pause",
                    "description": "Pause playback, keeping the current step position.",
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
                    "description": "Get current transport state (playing, bpm, current_step, current_pattern, playback_mode, arrangement_position)",
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
                    "description": "Get the full pattern grid showing all tracks and steps. Optionally specify a pattern slot (0-15) to view.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "integer",
                                "description": "Optional pattern slot index (0-15). If omitted, returns the active pattern."
                            }
                        }
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
                },
                {
                    "name": "select_pattern",
                    "description": "Switch the active pattern slot (0-15). When playing, the switch happens at the next pattern boundary.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "integer",
                                "description": "Pattern slot index (0-15)"
                            }
                        },
                        "required": ["pattern"]
                    }
                },
                {
                    "name": "get_pattern_bank",
                    "description": "Get an overview of all 16 pattern slots showing which have active steps.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "copy_pattern",
                    "description": "Copy a pattern from one slot to another.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "src": {
                                "type": "integer",
                                "description": "Source pattern slot (0-15)"
                            },
                            "dst": {
                                "type": "integer",
                                "description": "Destination pattern slot (0-15)"
                            }
                        },
                        "required": ["src", "dst"]
                    }
                },
                {
                    "name": "clear_pattern",
                    "description": "Clear all tracks in a pattern slot.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "integer",
                                "description": "Pattern slot index (0-15)"
                            }
                        },
                        "required": ["pattern"]
                    }
                },
                {
                    "name": "set_playback_mode",
                    "description": "Switch between pattern mode (loop single pattern) and song mode (play through arrangement).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "mode": {
                                "type": "string",
                                "description": "Playback mode: 'pattern' or 'song'"
                            }
                        },
                        "required": ["mode"]
                    }
                },
                {
                    "name": "get_arrangement",
                    "description": "Get the full arrangement (list of pattern entries with repeat counts).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "append_arrangement",
                    "description": "Add a pattern entry to the end of the arrangement.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "integer",
                                "description": "Pattern slot index (0-15)"
                            },
                            "repeats": {
                                "type": "integer",
                                "description": "Number of times to repeat (1-16, default: 1)"
                            }
                        },
                        "required": ["pattern"]
                    }
                },
                {
                    "name": "insert_arrangement",
                    "description": "Insert a pattern entry at a specific position in the arrangement.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "position": {
                                "type": "integer",
                                "description": "Position to insert at (0-based)"
                            },
                            "pattern": {
                                "type": "integer",
                                "description": "Pattern slot index (0-15)"
                            },
                            "repeats": {
                                "type": "integer",
                                "description": "Number of times to repeat (1-16, default: 1)"
                            }
                        },
                        "required": ["position", "pattern"]
                    }
                },
                {
                    "name": "remove_arrangement",
                    "description": "Remove an entry from the arrangement by position.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "position": {
                                "type": "integer",
                                "description": "Position to remove (0-based)"
                            }
                        },
                        "required": ["position"]
                    }
                },
                {
                    "name": "set_arrangement_entry",
                    "description": "Modify an existing arrangement entry's pattern and repeat count.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "position": {
                                "type": "integer",
                                "description": "Position to modify (0-based)"
                            },
                            "pattern": {
                                "type": "integer",
                                "description": "Pattern slot index (0-15)"
                            },
                            "repeats": {
                                "type": "integer",
                                "description": "Number of times to repeat (1-16)"
                            }
                        },
                        "required": ["position", "pattern", "repeats"]
                    }
                },
                {
                    "name": "clear_arrangement",
                    "description": "Remove all entries from the arrangement.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "save_project",
                    "description": "Save the current project state to a .grox JSON file.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "File path to save to (e.g., 'my_song.grox')"
                            }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "load_project",
                    "description": "Load a project from a .grox JSON file. Stops playback and replaces all state.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "File path to load from (e.g., 'my_song.grox')"
                            }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "export_wav",
                    "description": "Render and export audio as a WAV file (44100Hz, 16-bit stereo).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Output WAV file path (e.g., 'export.wav')"
                            },
                            "mode": {
                                "type": "string",
                                "description": "Export mode: 'pattern' (single pattern loop) or 'song' (full arrangement)"
                            },
                            "pattern": {
                                "type": "integer",
                                "description": "Pattern index (0-15) for pattern mode. Defaults to current pattern."
                            }
                        },
                        "required": ["path", "mode"]
                    }
                },
                {
                    "name": "list_projects",
                    "description": "List .grox project files in a directory.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "directory": {
                                "type": "string",
                                "description": "Directory to search (defaults to current directory)"
                            }
                        }
                    }
                }
            ]
        })
    }
}
