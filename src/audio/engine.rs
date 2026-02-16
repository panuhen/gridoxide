use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parking_lot::RwLock;
use serde_json::Value;

use crate::command::{Command, CommandReceiver};
use crate::fx::{
    configure_fx_chain, FxParamId, FxType, MasterFxParamId, MasterFxState, StereoReverb,
    TrackFxChain, TrackFxState,
};
use crate::sequencer::{
    Arrangement, Clock, Pattern, PatternBank, PlaybackMode, NUM_PATTERNS,
};
use crate::synth::{
    create_synth, SoundSource, SynthType,
};

/// Per-track state shared between audio thread and UI/MCP
#[derive(Clone, Debug)]
pub struct TrackState {
    pub synth_type: SynthType,
    pub name: String,
    pub default_note: u8,
    pub params_snapshot: Value,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub fx: TrackFxState,
}

/// Shared state between audio thread and UI/MCP
#[derive(Clone, Debug)]
pub struct SequencerState {
    pub playing: bool,
    pub bpm: f32,
    pub current_step: usize,
    pub pattern: Pattern,
    // Dynamic tracks
    pub tracks: Vec<TrackState>,
    // Master FX
    pub master_fx: MasterFxState,
    // Pattern bank + arrangement
    pub pattern_bank: PatternBank,
    pub current_pattern: usize,
    pub playback_mode: PlaybackMode,
    pub arrangement: Arrangement,
    pub arrangement_position: usize,
    pub arrangement_repeat: usize,
}

impl SequencerState {
    pub fn new() -> Self {
        let default_synths = [
            (SynthType::Kick, "KICK", 36u8),
            (SynthType::Snare, "SNARE", 50u8),
            (SynthType::HiHat, "HIHAT", 60u8),
            (SynthType::Bass, "BASS", 33u8),
        ];
        let tracks: Vec<TrackState> = default_synths
            .iter()
            .map(|(synth_type, name, default_note)| TrackState {
                synth_type: *synth_type,
                name: name.to_string(),
                default_note: *default_note,
                params_snapshot: Value::Null,
                volume: 0.8,
                pan: 0.0,
                mute: false,
                solo: false,
                fx: TrackFxState::default(),
            })
            .collect();

        Self {
            playing: false,
            bpm: 120.0,
            current_step: 0,
            pattern: Pattern::new(),
            tracks,
            master_fx: MasterFxState::default(),
            pattern_bank: PatternBank::new(),
            current_pattern: 0,
            playback_mode: PlaybackMode::Pattern,
            arrangement: Arrangement::new(),
            arrangement_position: 0,
            arrangement_repeat: 0,
        }
    }

    /// Number of tracks
    pub fn num_tracks(&self) -> usize {
        self.tracks.len()
    }
}

impl Default for SequencerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Audio engine managing the audio output stream and sequencer
pub struct AudioEngine {
    _stream: Stream,
    pub state: Arc<RwLock<SequencerState>>,
}

impl AudioEngine {
    /// Initialize the audio engine with default output device
    pub fn new(command_rx: CommandReceiver) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No output device available")?;

        let config = device.default_output_config()?;
        let state = Arc::new(RwLock::new(SequencerState::new()));

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                Self::build_stream::<f32>(&device, &config.into(), command_rx, state.clone())?
            }
            SampleFormat::I16 => {
                Self::build_stream::<i16>(&device, &config.into(), command_rx, state.clone())?
            }
            SampleFormat::U16 => {
                Self::build_stream::<u16>(&device, &config.into(), command_rx, state.clone())?
            }
            format => anyhow::bail!("Unsupported sample format: {:?}", format),
        };

        stream.play()?;

        Ok(Self {
            _stream: stream,
            state,
        })
    }

    /// Build the audio stream for a specific sample format
    fn build_stream<T>(
        device: &Device,
        config: &StreamConfig,
        command_rx: CommandReceiver,
        state: Arc<RwLock<SequencerState>>,
    ) -> Result<Stream>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let sample_rate = config.sample_rate.0 as f32;
        let channels = config.channels as usize;
        let num_tracks = 4usize; // default

        // Initialize synths dynamically
        let mut synths: Vec<Box<dyn SoundSource>> = vec![
            create_synth(SynthType::Kick, sample_rate, None),
            create_synth(SynthType::Snare, sample_rate, None),
            create_synth(SynthType::HiHat, sample_rate, None),
            create_synth(SynthType::Bass, sample_rate, None),
        ];

        // Initialize clock
        let mut clock = Clock::new(sample_rate, 120.0);

        // Local pattern copy (synced periodically from shared state)
        let mut pattern = Pattern::new();

        // Pattern bank + arrangement local state
        let mut local_pattern_bank = PatternBank::new();
        let mut local_current_pattern: usize = 0;
        let mut local_playback_mode = PlaybackMode::Pattern;
        let mut local_arrangement = Arrangement::new();
        let mut local_arrangement_position: usize = 0;
        let mut local_arrangement_repeat: usize = 0;
        let mut pending_pattern_switch: Option<usize> = None;

        // Local mixer state (dynamic)
        let mut local_volumes: Vec<f32> = vec![0.8; num_tracks];
        let mut local_pans: Vec<f32> = vec![0.0; num_tracks];
        let mut local_mutes: Vec<bool> = vec![false; num_tracks];
        let mut local_solos: Vec<bool> = vec![false; num_tracks];

        // Per-track FX chains
        let mut fx_chains: Vec<TrackFxChain> = (0..num_tracks)
            .map(|_| TrackFxChain::new(sample_rate))
            .collect();

        // Local FX state for syncing to shared state
        let mut local_track_fx: Vec<TrackFxState> = (0..num_tracks)
            .map(|_| TrackFxState::default())
            .collect();
        let mut local_master_fx = MasterFxState::default();

        // Master reverb
        let mut reverb = StereoReverb::new(sample_rate);
        let mut reverb_enabled = false;

        // Preview sample buffer (one-shot playback through master bus)
        let mut preview_buffer: Option<Vec<f32>> = None;
        let mut preview_pos: usize = 0;

        // For periodic state sync
        let mut sync_counter = 0usize;
        let sync_interval = (sample_rate / 60.0) as usize; // ~60 times per second

        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let num_synths = synths.len();

                // Process commands from the command bus
                while let Some((cmd, _source)) = command_rx.try_recv() {
                    match cmd {
                        Command::Play => {
                            clock.play();
                            if let Some(mut state) = state.try_write() {
                                state.playing = true;
                            }
                        }
                        Command::Pause => {
                            clock.pause();
                            if let Some(mut state) = state.try_write() {
                                state.playing = false;
                            }
                        }
                        Command::Stop => {
                            clock.stop();
                            // Apply any pending pattern switch immediately on stop
                            if let Some(new_pat) = pending_pattern_switch.take() {
                                local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();
                                local_current_pattern = new_pat;
                                pattern = local_pattern_bank.get(new_pat).clone();
                            }
                            // Reset song position
                            local_arrangement_position = 0;
                            local_arrangement_repeat = 0;
                            if let Some(mut state) = state.try_write() {
                                state.playing = false;
                                state.current_step = 0;
                                state.current_pattern = local_current_pattern;
                                state.pattern = pattern.clone();
                                state.arrangement_position = 0;
                                state.arrangement_repeat = 0;
                            }
                        }
                        Command::SetBpm(bpm) => {
                            clock.set_bpm(bpm);
                            if let Some(mut state) = state.try_write() {
                                state.bpm = clock.bpm();
                            }
                        }
                        Command::ToggleStep { track, step } => {
                            if track < num_synths {
                                pattern.toggle(track, step);
                                local_pattern_bank.get_mut(local_current_pattern).toggle(track, step);
                                if let Some(mut state) = state.try_write() {
                                    state.pattern = pattern.clone();
                                    state.pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();
                                }
                            }
                        }
                        Command::ClearTrack(track) => {
                            if track < num_synths {
                                pattern.clear_track(track);
                                local_pattern_bank.get_mut(local_current_pattern).clear_track(track);
                                if let Some(mut state) = state.try_write() {
                                    state.pattern = pattern.clone();
                                    state.pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();
                                }
                            }
                        }
                        Command::FillTrack(track) => {
                            if track < num_synths {
                                pattern.fill_track(track);
                                local_pattern_bank.get_mut(local_current_pattern).fill_track(track);
                                if let Some(mut state) = state.try_write() {
                                    state.pattern = pattern.clone();
                                    state.pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();
                                }
                            }
                        }
                        Command::SetStepNote { track, step, note } => {
                            if track < num_synths {
                                pattern.set_note(track, step, note);
                                local_pattern_bank.get_mut(local_current_pattern).set_note(track, step, note);
                                if let Some(mut state) = state.try_write() {
                                    state.pattern.set_note(track, step, note);
                                    state.pattern_bank.get_mut(local_current_pattern).set_note(track, step, note);
                                }
                            }
                        }
                        // Dynamic track parameter
                        Command::SetTrackParam { track, ref key, value } => {
                            if track < num_synths {
                                synths[track].set_param(key, value);
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].params_snapshot = synths[track].serialize_params();
                                }
                            }
                        }
                        Command::SetTrackVolume { track, volume } => {
                            if track < num_synths {
                                let v = volume.clamp(0.0, 1.0);
                                local_volumes[track] = v;
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].volume = v;
                                }
                            }
                        }
                        Command::SetTrackPan { track, pan } => {
                            if track < num_synths {
                                let p = pan.clamp(-1.0, 1.0);
                                local_pans[track] = p;
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].pan = p;
                                }
                            }
                        }
                        Command::ToggleMute(track) => {
                            if track < num_synths {
                                local_mutes[track] = !local_mutes[track];
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].mute = local_mutes[track];
                                }
                            }
                        }
                        Command::ToggleSolo(track) => {
                            if track < num_synths {
                                local_solos[track] = !local_solos[track];
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].solo = local_solos[track];
                                }
                            }
                        }
                        // Per-track FX commands
                        Command::SetFxParam { track, param, value } => {
                            if track < num_synths {
                                apply_fx_param(&mut fx_chains[track], &mut local_track_fx[track], param, value);
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].fx = local_track_fx[track].clone();
                                }
                            }
                        }
                        Command::SetFxFilterType { track, filter_type } => {
                            if track < num_synths {
                                fx_chains[track].filter.set_filter_type(filter_type);
                                local_track_fx[track].filter_type = filter_type;
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].fx.filter_type = filter_type;
                                }
                            }
                        }
                        Command::ToggleFxEnabled { track, fx } => {
                            if track < num_synths {
                                match fx {
                                    FxType::Filter => {
                                        fx_chains[track].filter_enabled = !fx_chains[track].filter_enabled;
                                        local_track_fx[track].filter_enabled = fx_chains[track].filter_enabled;
                                    }
                                    FxType::Distortion => {
                                        fx_chains[track].dist_enabled = !fx_chains[track].dist_enabled;
                                        local_track_fx[track].dist_enabled = fx_chains[track].dist_enabled;
                                    }
                                    FxType::Delay => {
                                        fx_chains[track].delay_enabled = !fx_chains[track].delay_enabled;
                                        local_track_fx[track].delay_enabled = fx_chains[track].delay_enabled;
                                    }
                                }
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].fx = local_track_fx[track].clone();
                                }
                            }
                        }
                        // Master FX commands
                        Command::SetMasterFxParam { param, value } => {
                            apply_master_fx_param(&mut reverb, &mut local_master_fx, param, value);
                            reverb_enabled = local_master_fx.reverb_enabled;
                            if let Some(mut state) = state.try_write() {
                                state.master_fx = local_master_fx.clone();
                            }
                        }
                        Command::ToggleMasterFxEnabled => {
                            reverb_enabled = !reverb_enabled;
                            local_master_fx.reverb_enabled = reverb_enabled;
                            if let Some(mut state) = state.try_write() {
                                state.master_fx.reverb_enabled = reverb_enabled;
                            }
                        }

                        // Pattern Bank commands
                        Command::SelectPattern(p) => {
                            if p < NUM_PATTERNS {
                                // Save current pattern to bank
                                local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();

                                if clock.is_playing() {
                                    // Queue for boundary switch
                                    pending_pattern_switch = Some(p);
                                } else {
                                    // Apply immediately when stopped
                                    local_current_pattern = p;
                                    pattern = local_pattern_bank.get(p).clone();
                                    pending_pattern_switch = None;
                                }

                                if let Some(mut state) = state.try_write() {
                                    state.pattern_bank = local_pattern_bank.clone();
                                    if !clock.is_playing() {
                                        state.current_pattern = p;
                                        state.pattern = pattern.clone();
                                    }
                                }
                            }
                        }
                        Command::CopyPattern { src, dst } => {
                            if src < NUM_PATTERNS && dst < NUM_PATTERNS {
                                let src_pattern = local_pattern_bank.get(src).clone();
                                *local_pattern_bank.get_mut(dst) = src_pattern;
                                // If we copied into the active pattern, update local
                                if dst == local_current_pattern {
                                    pattern = local_pattern_bank.get(dst).clone();
                                }
                                if let Some(mut state) = state.try_write() {
                                    state.pattern_bank = local_pattern_bank.clone();
                                    if dst == local_current_pattern {
                                        state.pattern = pattern.clone();
                                    }
                                }
                            }
                        }
                        Command::ClearPattern(p) => {
                            if p < NUM_PATTERNS {
                                local_pattern_bank.get_mut(p).clear_all();
                                if p == local_current_pattern {
                                    pattern = local_pattern_bank.get(p).clone();
                                }
                                if let Some(mut state) = state.try_write() {
                                    state.pattern_bank = local_pattern_bank.clone();
                                    if p == local_current_pattern {
                                        state.pattern = pattern.clone();
                                    }
                                }
                            }
                        }

                        // Playback mode
                        Command::SetPlaybackMode(mode) => {
                            local_playback_mode = mode;
                            if mode == PlaybackMode::Song {
                                local_arrangement_position = 0;
                                local_arrangement_repeat = 0;
                            }
                            if let Some(mut state) = state.try_write() {
                                state.playback_mode = mode;
                                state.arrangement_position = local_arrangement_position;
                                state.arrangement_repeat = local_arrangement_repeat;
                            }
                        }

                        // Arrangement commands
                        Command::AppendArrangement { pattern: p, repeats } => {
                            local_arrangement.append(p, repeats);
                            if let Some(mut state) = state.try_write() {
                                state.arrangement = local_arrangement.clone();
                            }
                        }
                        Command::InsertArrangement { position, pattern: p, repeats } => {
                            local_arrangement.insert(position, p, repeats);
                            if let Some(mut state) = state.try_write() {
                                state.arrangement = local_arrangement.clone();
                            }
                        }
                        Command::RemoveArrangement(pos) => {
                            local_arrangement.remove(pos);
                            // Adjust position if needed
                            if local_arrangement_position >= local_arrangement.len() && local_arrangement.len() > 0 {
                                local_arrangement_position = local_arrangement.len() - 1;
                            }
                            if let Some(mut state) = state.try_write() {
                                state.arrangement = local_arrangement.clone();
                                state.arrangement_position = local_arrangement_position;
                            }
                        }
                        Command::SetArrangementEntry { position, pattern: p, repeats } => {
                            local_arrangement.set_entry(position, p, repeats);
                            if let Some(mut state) = state.try_write() {
                                state.arrangement = local_arrangement.clone();
                            }
                        }
                        Command::ClearArrangement => {
                            local_arrangement.clear();
                            local_arrangement_position = 0;
                            local_arrangement_repeat = 0;
                            if let Some(mut state) = state.try_write() {
                                state.arrangement = local_arrangement.clone();
                                state.arrangement_position = 0;
                                state.arrangement_repeat = 0;
                            }
                        }

                        Command::AddTrack { synth_type, ref name } => {
                            if !clock.is_playing() {
                                let new_synth = create_synth(synth_type, sample_rate, None);
                                let default_note = new_synth.default_note();
                                synths.push(new_synth);
                                local_volumes.push(0.8);
                                local_pans.push(0.0);
                                local_mutes.push(false);
                                local_solos.push(false);
                                fx_chains.push(TrackFxChain::new(sample_rate));
                                local_track_fx.push(TrackFxState::default());
                                // Add track to all patterns
                                for pat in local_pattern_bank.patterns.iter_mut() {
                                    pat.add_track(default_note);
                                }
                                pattern = local_pattern_bank.get(local_current_pattern).clone();
                                if let Some(mut state) = state.try_write() {
                                    state.tracks.push(TrackState {
                                        synth_type,
                                        name: name.clone(),
                                        default_note,
                                        params_snapshot: synths.last().unwrap().serialize_params(),
                                        volume: 0.8,
                                        pan: 0.0,
                                        mute: false,
                                        solo: false,
                                        fx: TrackFxState::default(),
                                    });
                                    state.pattern_bank = local_pattern_bank.clone();
                                    state.pattern = pattern.clone();
                                }
                            }
                        }

                        Command::RemoveTrack(track) => {
                            if !clock.is_playing() && track < synths.len() && synths.len() > 1 {
                                synths.remove(track);
                                local_volumes.remove(track);
                                local_pans.remove(track);
                                local_mutes.remove(track);
                                local_solos.remove(track);
                                fx_chains.remove(track);
                                local_track_fx.remove(track);
                                // Remove track from all patterns
                                for pat in local_pattern_bank.patterns.iter_mut() {
                                    pat.remove_track(track);
                                }
                                pattern = local_pattern_bank.get(local_current_pattern).clone();
                                if let Some(mut state) = state.try_write() {
                                    state.tracks.remove(track);
                                    state.pattern_bank = local_pattern_bank.clone();
                                    state.pattern = pattern.clone();
                                }
                            }
                        }

                        Command::LoadSample { track, buffer, ref path } => {
                            if track < synths.len() {
                                // Convert non-sampler tracks to sampler
                                if synths[track].synth_type() != SynthType::Sampler {
                                    synths[track] = create_synth(SynthType::Sampler, sample_rate, None);
                                    if let Some(mut state) = state.try_write() {
                                        state.tracks[track].synth_type = SynthType::Sampler;
                                    }
                                }
                                synths[track].load_buffer(buffer, path);
                                if let Some(mut state) = state.try_write() {
                                    state.tracks[track].params_snapshot = synths[track].serialize_params();
                                }
                            }
                        }

                        Command::PreviewSample(buffer) => {
                            preview_buffer = Some(buffer);
                            preview_pos = 0;
                        }

                        Command::LoadProject(new_state) => {
                            // Stop playback
                            clock.stop();
                            clock.set_bpm(new_state.bpm);
                            pending_pattern_switch = None;

                            // Reconstruct synths from track data
                            synths.clear();
                            local_volumes.clear();
                            local_pans.clear();
                            local_mutes.clear();
                            local_solos.clear();
                            fx_chains.clear();
                            local_track_fx.clear();

                            for track in &new_state.tracks {
                                let synth = create_synth(
                                    track.synth_type,
                                    sample_rate,
                                    Some(&track.params_snapshot),
                                );
                                synths.push(synth);
                                local_volumes.push(track.volume);
                                local_pans.push(track.pan);
                                local_mutes.push(track.mute);
                                local_solos.push(track.solo);
                                let mut chain = TrackFxChain::new(sample_rate);
                                configure_fx_chain(&mut chain, &track.fx);
                                fx_chains.push(chain);
                                local_track_fx.push(track.fx.clone());
                            }

                            // Restore master FX
                            reverb.set_decay(new_state.master_fx.reverb_decay);
                            reverb.set_mix(new_state.master_fx.reverb_mix);
                            reverb.set_damping(new_state.master_fx.reverb_damping);
                            reverb_enabled = new_state.master_fx.reverb_enabled;
                            local_master_fx = new_state.master_fx.clone();

                            // Restore pattern bank + arrangement
                            local_pattern_bank = new_state.pattern_bank.clone();
                            local_current_pattern = new_state.current_pattern;
                            pattern = local_pattern_bank.get(local_current_pattern).clone();
                            local_playback_mode = new_state.playback_mode;
                            local_arrangement = new_state.arrangement.clone();
                            local_arrangement_position = 0;
                            local_arrangement_repeat = 0;

                            // Sync shared state
                            if let Some(mut state) = state.try_write() {
                                *state = *new_state;
                                state.playing = false;
                                state.current_step = 0;
                                state.arrangement_position = 0;
                                state.arrangement_repeat = 0;
                            }
                        }
                    }
                }

                // Generate audio
                for frame in data.chunks_mut(channels) {
                    let num_synths = synths.len();

                    // Check for step trigger
                    if let Some(step) = clock.tick() {
                        // Notify all synths of step tick (for hold_steps countdown)
                        for synth in synths.iter_mut() {
                            synth.step_tick();
                        }
                        // Trigger synths based on pattern
                        for i in 0..num_synths {
                            let sd = pattern.get_step(i, step);
                            if sd.active {
                                synths[i].trigger_with_note(sd.note);
                            }
                        }
                    }

                    // Pattern boundary logic
                    if clock.take_pattern_wrap() {
                        match local_playback_mode {
                            PlaybackMode::Pattern => {
                                // Apply pending pattern switch at boundary
                                if let Some(new_pat) = pending_pattern_switch.take() {
                                    local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();
                                    local_current_pattern = new_pat;
                                    pattern = local_pattern_bank.get(new_pat).clone();
                                    if let Some(mut state) = state.try_write() {
                                        state.current_pattern = new_pat;
                                        state.pattern = pattern.clone();
                                        state.pattern_bank = local_pattern_bank.clone();
                                    }
                                }
                            }
                            PlaybackMode::Song => {
                                if !local_arrangement.is_empty() {
                                    let entry = local_arrangement.entries[local_arrangement_position];
                                    local_arrangement_repeat += 1;
                                    if local_arrangement_repeat >= entry.repeats {
                                        // Advance to next entry
                                        local_arrangement_repeat = 0;
                                        local_arrangement_position = (local_arrangement_position + 1)
                                            % local_arrangement.len();
                                        // Load new pattern from bank
                                        let new_entry = local_arrangement.entries[local_arrangement_position];
                                        local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps.clone();
                                        local_current_pattern = new_entry.pattern;
                                        pattern = local_pattern_bank.get(new_entry.pattern).clone();
                                        if let Some(mut state) = state.try_write() {
                                            state.current_pattern = local_current_pattern;
                                            state.pattern = pattern.clone();
                                            state.arrangement_position = local_arrangement_position;
                                            state.arrangement_repeat = local_arrangement_repeat;
                                        }
                                    } else if let Some(mut state) = state.try_write() {
                                        state.arrangement_repeat = local_arrangement_repeat;
                                    }
                                }
                            }
                        }
                    }

                    // Get raw synth output and apply per-track FX
                    let any_solo = local_solos.iter().any(|&s| s);

                    let mut left = 0.0f32;
                    let mut right = 0.0f32;
                    for i in 0..num_synths {
                        let raw = fx_chains[i].process(synths[i].next_sample());
                        let audible = if any_solo {
                            local_solos[i]
                        } else {
                            !local_mutes[i]
                        };
                        if !audible {
                            continue;
                        }
                        let s = raw * local_volumes[i];
                        let angle = (local_pans[i] + 1.0) * 0.25 * std::f32::consts::PI;
                        left += s * angle.cos();
                        right += s * angle.sin();
                    }

                    // Preview sample (one-shot, no FX, straight to mix)
                    if let Some(ref buf) = preview_buffer {
                        if preview_pos < buf.len() {
                            let preview_sample = buf[preview_pos] * 0.8;
                            left += preview_sample;
                            right += preview_sample;
                            preview_pos += 1;
                        } else {
                            preview_buffer = None;
                            preview_pos = 0;
                        }
                    }

                    // Master reverb
                    if reverb_enabled {
                        let (rl, rr) = reverb.process_stereo(left, right);
                        left = rl;
                        right = rr;
                    }

                    // Soft clip both channels
                    left = soft_clip(left);
                    right = soft_clip(right);

                    // Write stereo output (left to ch0, right to ch1, mono fallback for others)
                    for (ch, channel_sample) in frame.iter_mut().enumerate() {
                        let sample = match ch {
                            0 => left,
                            1 => right,
                            _ => (left + right) * 0.5,
                        };
                        *channel_sample = T::from_sample(sample);
                    }

                    // Periodic state sync (for UI to read current_step + params snapshots)
                    sync_counter += 1;
                    if sync_counter >= sync_interval {
                        sync_counter = 0;
                        if let Some(mut state) = state.try_write() {
                            state.current_step = clock.current_step();
                            state.playing = clock.is_playing();
                            state.pattern = pattern.clone();
                            state.current_pattern = local_current_pattern;
                            state.playback_mode = local_playback_mode;
                            state.arrangement_position = local_arrangement_position;
                            state.arrangement_repeat = local_arrangement_repeat;
                            // Sync param snapshots
                            for (i, synth) in synths.iter().enumerate() {
                                if i < state.tracks.len() {
                                    state.tracks[i].params_snapshot = synth.serialize_params();
                                }
                            }
                        }
                    }
                }
            },
            |err| {
                eprintln!("Audio stream error: {}", err);
            },
            None,
        )?;

        Ok(stream)
    }
}

/// Apply a per-track FX parameter change
fn apply_fx_param(chain: &mut TrackFxChain, local: &mut TrackFxState, param: FxParamId, value: f32) {
    match param {
        FxParamId::FilterCutoff => {
            let v = value.clamp(20.0, 20000.0);
            chain.filter.set_cutoff(v);
            local.filter_cutoff = v;
        }
        FxParamId::FilterResonance => {
            let v = value.clamp(0.0, 0.95);
            chain.filter.set_resonance(v);
            local.filter_resonance = v;
        }
        FxParamId::DistDrive => {
            let v = value.clamp(0.0, 1.0);
            chain.distortion.set_drive(v);
            local.dist_drive = v;
        }
        FxParamId::DistMix => {
            let v = value.clamp(0.0, 1.0);
            chain.distortion.set_mix(v);
            local.dist_mix = v;
        }
        FxParamId::DelayTime => {
            let v = value.clamp(10.0, 500.0);
            chain.delay.set_time(v);
            local.delay_time = v;
        }
        FxParamId::DelayFeedback => {
            let v = value.clamp(0.0, 0.9);
            chain.delay.set_feedback(v);
            local.delay_feedback = v;
        }
        FxParamId::DelayMix => {
            let v = value.clamp(0.0, 1.0);
            chain.delay.set_mix(v);
            local.delay_mix = v;
        }
    }
}

/// Apply a master FX parameter change
fn apply_master_fx_param(reverb: &mut StereoReverb, local: &mut MasterFxState, param: MasterFxParamId, value: f32) {
    match param {
        MasterFxParamId::ReverbDecay => {
            let v = value.clamp(0.1, 0.95);
            reverb.set_decay(v);
            local.reverb_decay = v;
        }
        MasterFxParamId::ReverbMix => {
            let v = value.clamp(0.0, 1.0);
            reverb.set_mix(v);
            local.reverb_mix = v;
        }
        MasterFxParamId::ReverbDamping => {
            let v = value.clamp(0.0, 1.0);
            reverb.set_damping(v);
            local.reverb_damping = v;
        }
    }
}

/// Soft clipping function to prevent harsh digital clipping
fn soft_clip(x: f32) -> f32 {
    if x > 1.0 {
        1.0 - (-x + 1.0).exp() * 0.5
    } else if x < -1.0 {
        -1.0 + (x + 1.0).exp() * 0.5
    } else {
        x
    }
}
