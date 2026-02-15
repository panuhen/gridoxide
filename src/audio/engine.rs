use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parking_lot::RwLock;

use crate::command::{Command, CommandReceiver};
use crate::fx::{
    configure_fx_chain, FxParamId, FxType, MasterFxParamId, MasterFxState, StereoReverb,
    TrackFxChain, TrackFxState,
};
use crate::sequencer::{
    Arrangement, Clock, Pattern, PatternBank, PlaybackMode, NUM_PATTERNS,
};
use crate::synth::{
    BassParams, BassSynth, HiHatParams, HiHatSynth, KickParams, KickSynth, ParamId, SnareParams,
    SnareSynth,
};

/// Shared state between audio thread and UI/MCP
#[derive(Clone, Debug)]
pub struct SequencerState {
    pub playing: bool,
    pub bpm: f32,
    pub current_step: usize,
    pub pattern: Pattern,
    // Synth parameters
    pub kick_params: KickParams,
    pub snare_params: SnareParams,
    pub hihat_params: HiHatParams,
    pub bass_params: BassParams,
    // Mixer state
    pub track_volumes: [f32; 4],
    pub track_pans: [f32; 4],
    pub track_mutes: [bool; 4],
    pub track_solos: [bool; 4],
    // FX state
    pub track_fx: [TrackFxState; 4],
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
        Self {
            playing: false,
            bpm: 120.0,
            current_step: 0,
            pattern: Pattern::new(),
            kick_params: KickParams::default(),
            snare_params: SnareParams::default(),
            hihat_params: HiHatParams::default(),
            bass_params: BassParams::default(),
            track_volumes: [0.8; 4],
            track_pans: [0.0; 4],
            track_mutes: [false; 4],
            track_solos: [false; 4],
            track_fx: [
                TrackFxState::default(),
                TrackFxState::default(),
                TrackFxState::default(),
                TrackFxState::default(),
            ],
            master_fx: MasterFxState::default(),
            pattern_bank: PatternBank::new(),
            current_pattern: 0,
            playback_mode: PlaybackMode::Pattern,
            arrangement: Arrangement::new(),
            arrangement_position: 0,
            arrangement_repeat: 0,
        }
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

        // Initialize synths
        let mut kick = KickSynth::new(sample_rate);
        let mut snare = SnareSynth::new(sample_rate);
        let mut hihat = HiHatSynth::new(sample_rate);
        let mut bass = BassSynth::new(sample_rate);

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

        // Local mixer state
        let mut local_volumes = [0.8f32; 4];
        let mut local_pans = [0.0f32; 4];
        let mut local_mutes = [false; 4];
        let mut local_solos = [false; 4];

        // Per-track FX chains
        let mut fx_chains = [
            TrackFxChain::new(sample_rate),
            TrackFxChain::new(sample_rate),
            TrackFxChain::new(sample_rate),
            TrackFxChain::new(sample_rate),
        ];

        // Local FX state for syncing to shared state
        let mut local_track_fx = [
            TrackFxState::default(),
            TrackFxState::default(),
            TrackFxState::default(),
            TrackFxState::default(),
        ];
        let mut local_master_fx = MasterFxState::default();

        // Master reverb
        let mut reverb = StereoReverb::new(sample_rate);
        let mut reverb_enabled = false;

        // For periodic state sync
        let mut sync_counter = 0usize;
        let sync_interval = (sample_rate / 60.0) as usize; // ~60 times per second

        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
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
                                local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;
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
                            pattern.toggle(track, step);
                            local_pattern_bank.get_mut(local_current_pattern).toggle(track, step);
                            if let Some(mut state) = state.try_write() {
                                state.pattern = pattern.clone();
                                state.pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;
                            }
                        }
                        Command::ClearTrack(track) => {
                            pattern.clear_track(track);
                            local_pattern_bank.get_mut(local_current_pattern).clear_track(track);
                            if let Some(mut state) = state.try_write() {
                                state.pattern = pattern.clone();
                                state.pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;
                            }
                        }
                        Command::FillTrack(track) => {
                            pattern.fill_track(track);
                            local_pattern_bank.get_mut(local_current_pattern).fill_track(track);
                            if let Some(mut state) = state.try_write() {
                                state.pattern = pattern.clone();
                                state.pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;
                            }
                        }
                        Command::SetStepNote { track, step, note } => {
                            pattern.set_note(track, step, note);
                            local_pattern_bank.get_mut(local_current_pattern).set_note(track, step, note);
                            if let Some(mut state) = state.try_write() {
                                state.pattern.set_note(track, step, note);
                                state.pattern_bank.get_mut(local_current_pattern).set_note(track, step, note);
                            }
                        }
                        // Synth parameter commands
                        Command::SetKickParams(params) => {
                            kick.set_params(params.clone());
                            if let Some(mut state) = state.try_write() {
                                state.kick_params = params;
                            }
                        }
                        Command::SetSnareParams(params) => {
                            snare.set_params(params.clone());
                            if let Some(mut state) = state.try_write() {
                                state.snare_params = params;
                            }
                        }
                        Command::SetHiHatParams(params) => {
                            hihat.set_params(params.clone());
                            if let Some(mut state) = state.try_write() {
                                state.hihat_params = params;
                            }
                        }
                        Command::SetBassParams(params) => {
                            bass.set_params(params.clone());
                            if let Some(mut state) = state.try_write() {
                                state.bass_params = params;
                            }
                        }
                        Command::SetParam { param, value } => {
                            // Apply single parameter change
                            apply_param(
                                &mut kick, &mut snare, &mut hihat, &mut bass, param, value,
                            );
                            // Update shared state
                            if let Some(mut state) = state.try_write() {
                                update_state_param(&mut state, param, value);
                            }
                        }
                        Command::SetTrackVolume { track, volume } => {
                            if track < 4 {
                                let v = volume.clamp(0.0, 1.0);
                                local_volumes[track] = v;
                                if let Some(mut state) = state.try_write() {
                                    state.track_volumes[track] = v;
                                }
                            }
                        }
                        Command::SetTrackPan { track, pan } => {
                            if track < 4 {
                                let p = pan.clamp(-1.0, 1.0);
                                local_pans[track] = p;
                                if let Some(mut state) = state.try_write() {
                                    state.track_pans[track] = p;
                                }
                            }
                        }
                        Command::ToggleMute(track) => {
                            if track < 4 {
                                local_mutes[track] = !local_mutes[track];
                                if let Some(mut state) = state.try_write() {
                                    state.track_mutes[track] = local_mutes[track];
                                }
                            }
                        }
                        Command::ToggleSolo(track) => {
                            if track < 4 {
                                local_solos[track] = !local_solos[track];
                                if let Some(mut state) = state.try_write() {
                                    state.track_solos[track] = local_solos[track];
                                }
                            }
                        }
                        // Per-track FX commands
                        Command::SetFxParam { track, param, value } => {
                            if track < 4 {
                                apply_fx_param(&mut fx_chains[track], &mut local_track_fx[track], param, value);
                                if let Some(mut state) = state.try_write() {
                                    state.track_fx[track] = local_track_fx[track].clone();
                                }
                            }
                        }
                        Command::SetFxFilterType { track, filter_type } => {
                            if track < 4 {
                                fx_chains[track].filter.set_filter_type(filter_type);
                                local_track_fx[track].filter_type = filter_type;
                                if let Some(mut state) = state.try_write() {
                                    state.track_fx[track].filter_type = filter_type;
                                }
                            }
                        }
                        Command::ToggleFxEnabled { track, fx } => {
                            if track < 4 {
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
                                    state.track_fx[track] = local_track_fx[track].clone();
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
                                local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;

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

                        Command::LoadProject(new_state) => {
                            // Stop playback
                            clock.stop();
                            clock.set_bpm(new_state.bpm);
                            pending_pattern_switch = None;

                            // Restore synth params
                            kick.set_params(new_state.kick_params.clone());
                            snare.set_params(new_state.snare_params.clone());
                            hihat.set_params(new_state.hihat_params.clone());
                            bass.set_params(new_state.bass_params.clone());

                            // Restore mixer
                            local_volumes = new_state.track_volumes;
                            local_pans = new_state.track_pans;
                            local_mutes = new_state.track_mutes;
                            local_solos = new_state.track_solos;

                            // Restore FX
                            for i in 0..4 {
                                configure_fx_chain(&mut fx_chains[i], &new_state.track_fx[i]);
                                local_track_fx[i] = new_state.track_fx[i].clone();
                            }
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
                    // Check for step trigger
                    if let Some(step) = clock.tick() {
                        // Trigger synths based on pattern, passing per-step notes
                        let s0 = pattern.get_step(0, step);
                        if s0.active {
                            kick.trigger_with_note(s0.note);
                        }
                        let s1 = pattern.get_step(1, step);
                        if s1.active {
                            snare.trigger_with_note(s1.note);
                        }
                        let s2 = pattern.get_step(2, step);
                        if s2.active {
                            hihat.trigger_with_note(s2.note);
                        }
                        let s3 = pattern.get_step(3, step);
                        if s3.active {
                            bass.trigger_with_note(s3.note);
                        }
                    }

                    // Pattern boundary logic
                    if clock.take_pattern_wrap() {
                        match local_playback_mode {
                            PlaybackMode::Pattern => {
                                // Apply pending pattern switch at boundary
                                if let Some(new_pat) = pending_pattern_switch.take() {
                                    local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;
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
                                        local_pattern_bank.get_mut(local_current_pattern).steps = pattern.steps;
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
                    let raw = [
                        fx_chains[0].process(kick.next_sample()),
                        fx_chains[1].process(snare.next_sample()),
                        fx_chains[2].process(hihat.next_sample()),
                        fx_chains[3].process(bass.next_sample()),
                    ];

                    // Mix with per-track volume, pan, mute, solo
                    let any_solo = local_solos.iter().any(|&s| s);

                    let mut left = 0.0f32;
                    let mut right = 0.0f32;
                    for i in 0..4 {
                        let audible = if any_solo {
                            local_solos[i]
                        } else {
                            !local_mutes[i]
                        };
                        if !audible {
                            continue;
                        }
                        let s = raw[i] * local_volumes[i];
                        // Equal-power pan: pan -1.0 = full left, 0.0 = center, 1.0 = full right
                        let angle = (local_pans[i] + 1.0) * 0.25 * std::f32::consts::PI;
                        left += s * angle.cos();
                        right += s * angle.sin();
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

                    // Periodic state sync (for UI to read current_step + full pattern)
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

/// Apply a single parameter change to the appropriate synth
fn apply_param(
    kick: &mut KickSynth,
    snare: &mut SnareSynth,
    hihat: &mut HiHatSynth,
    bass: &mut BassSynth,
    param: ParamId,
    value: f32,
) {
    match param {
        // Kick parameters
        ParamId::KickPitchStart => {
            let mut p = kick.params().clone();
            p.pitch_start = value;
            kick.set_params(p);
        }
        ParamId::KickPitchEnd => {
            let mut p = kick.params().clone();
            p.pitch_end = value;
            kick.set_params(p);
        }
        ParamId::KickPitchDecay => {
            let mut p = kick.params().clone();
            p.pitch_decay = value;
            kick.set_params(p);
        }
        ParamId::KickAmpDecay => {
            let mut p = kick.params().clone();
            p.amp_decay = value;
            kick.set_params(p);
        }
        ParamId::KickClick => {
            let mut p = kick.params().clone();
            p.click = value;
            kick.set_params(p);
        }
        ParamId::KickDrive => {
            let mut p = kick.params().clone();
            p.drive = value;
            kick.set_params(p);
        }
        // Snare parameters
        ParamId::SnareToneFreq => {
            let mut p = snare.params().clone();
            p.tone_freq = value;
            snare.set_params(p);
        }
        ParamId::SnareToneDecay => {
            let mut p = snare.params().clone();
            p.tone_decay = value;
            snare.set_params(p);
        }
        ParamId::SnareNoiseDecay => {
            let mut p = snare.params().clone();
            p.noise_decay = value;
            snare.set_params(p);
        }
        ParamId::SnareToneMix => {
            let mut p = snare.params().clone();
            p.tone_mix = value;
            snare.set_params(p);
        }
        ParamId::SnareSnappy => {
            let mut p = snare.params().clone();
            p.snappy = value;
            snare.set_params(p);
        }
        // HiHat parameters
        ParamId::HiHatDecay => {
            let mut p = hihat.params().clone();
            p.decay = value;
            hihat.set_params(p);
        }
        ParamId::HiHatTone => {
            let mut p = hihat.params().clone();
            p.tone = value;
            hihat.set_params(p);
        }
        ParamId::HiHatOpen => {
            let mut p = hihat.params().clone();
            p.open = value;
            hihat.set_params(p);
        }
        // Bass parameters
        ParamId::BassFrequency => {
            let mut p = bass.params().clone();
            p.frequency = value;
            bass.set_params(p);
        }
        ParamId::BassDecay => {
            let mut p = bass.params().clone();
            p.decay = value;
            bass.set_params(p);
        }
        ParamId::BassSawMix => {
            let mut p = bass.params().clone();
            p.saw_mix = value;
            bass.set_params(p);
        }
        ParamId::BassSub => {
            let mut p = bass.params().clone();
            p.sub = value;
            bass.set_params(p);
        }
    }
}

/// Update shared state with a single parameter change
fn update_state_param(state: &mut SequencerState, param: ParamId, value: f32) {
    match param {
        ParamId::KickPitchStart => state.kick_params.pitch_start = value,
        ParamId::KickPitchEnd => state.kick_params.pitch_end = value,
        ParamId::KickPitchDecay => state.kick_params.pitch_decay = value,
        ParamId::KickAmpDecay => state.kick_params.amp_decay = value,
        ParamId::KickClick => state.kick_params.click = value,
        ParamId::KickDrive => state.kick_params.drive = value,
        ParamId::SnareToneFreq => state.snare_params.tone_freq = value,
        ParamId::SnareToneDecay => state.snare_params.tone_decay = value,
        ParamId::SnareNoiseDecay => state.snare_params.noise_decay = value,
        ParamId::SnareToneMix => state.snare_params.tone_mix = value,
        ParamId::SnareSnappy => state.snare_params.snappy = value,
        ParamId::HiHatDecay => state.hihat_params.decay = value,
        ParamId::HiHatTone => state.hihat_params.tone = value,
        ParamId::HiHatOpen => state.hihat_params.open = value,
        ParamId::BassFrequency => state.bass_params.frequency = value,
        ParamId::BassDecay => state.bass_params.decay = value,
        ParamId::BassSawMix => state.bass_params.saw_mix = value,
        ParamId::BassSub => state.bass_params.sub = value,
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
