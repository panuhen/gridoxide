use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parking_lot::RwLock;

use crate::command::{Command, CommandReceiver};
use crate::fx::{
    FxParamId, FxType, MasterFxParamId, MasterFxState, StereoReverb, TrackFxChain, TrackFxState,
};
use crate::sequencer::{Clock, Pattern};
use crate::synth::{
    BassParams, BassSynth, HiHatParams, HiHatSynth, KickParams, KickSynth, ParamId, SnareParams,
    SnareSynth,
};

/// Shared state between audio thread and UI/MCP
#[derive(Clone)]
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
                        Command::Stop => {
                            clock.stop();
                            if let Some(mut state) = state.try_write() {
                                state.playing = false;
                                state.current_step = 0;
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
                            if let Some(mut state) = state.try_write() {
                                state.pattern = pattern.clone();
                            }
                        }
                        Command::ClearTrack(track) => {
                            pattern.clear_track(track);
                            if let Some(mut state) = state.try_write() {
                                state.pattern = pattern.clone();
                            }
                        }
                        Command::FillTrack(track) => {
                            pattern.fill_track(track);
                            if let Some(mut state) = state.try_write() {
                                state.pattern = pattern.clone();
                            }
                        }
                        Command::SetStepNote { track, step, note } => {
                            pattern.set_note(track, step, note);
                            if let Some(mut state) = state.try_write() {
                                state.pattern.set_note(track, step, note);
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
