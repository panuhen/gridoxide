use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::source::{ParamDescriptor, SoundSource, SynthType};

/// Sampler synth parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SamplerParams {
    pub amplitude: f32,    // 0.0-1.0, default 0.8
    pub attack: f32,       // 0-50 ms, default 0
    pub decay: f32,        // 10-500 ms, time to reach sustain level, default 100
    pub sustain: f32,      // 0.0-1.0, sustain level, default 0.8
    pub release: f32,      // 10-2000 ms, release time, default 200
    pub start_point: f32,  // 0.0-1.0 (fraction of buffer), default 0.0
    pub end_point: f32,    // 0.0-1.0 (fraction of buffer), default 1.0
    pub pitch_shift: f32,  // -24 to +24 semitones, default 0
    #[serde(default)]
    pub loop_enabled: bool, // default false (one-shot)
    #[serde(default = "default_loop_end")]
    pub loop_start: f32,   // 0.0-1.0, default 0.0
    #[serde(default = "default_loop_end")]
    pub loop_end: f32,     // 0.0-1.0, default 1.0
    #[serde(default = "default_hold_steps")]
    pub hold_steps: u8,    // 1-16, default 4
    #[serde(default)]
    pub wav_path: Option<String>, // for display and serialization
}

fn default_loop_end() -> f32 {
    1.0
}

fn default_hold_steps() -> u8 {
    4
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            amplitude: 0.8,
            attack: 0.0,
            decay: 100.0,
            sustain: 0.8,
            release: 200.0,
            start_point: 0.0,
            end_point: 1.0,
            pitch_shift: 0.0,
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 1.0,
            hold_steps: 4,
            wav_path: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EnvelopePhase {
    Off,
    Attack,  // 0 → 1.0 over attack time
    Decay,   // 1.0 → sustain level over decay time
    Sustain, // Hold at sustain level (loop continues)
    Release, // sustain → 0 over release time (triggered by note_off or hold_steps)
}

/// Sampler synth: plays back a WAV buffer with pitch shifting
pub struct SamplerSynth {
    sample_rate: f32,
    buffer: Vec<f32>,           // mono f32 sample data
    position: Option<f64>,      // None = not playing, Some = current fractional position
    playback_rate: f64,         // computed from note + pitch_shift
    envelope: f32,              // current envelope value (0.0-1.0)
    envelope_phase: EnvelopePhase,
    envelope_samples: usize,    // samples elapsed in current phase
    release_start_level: f32,   // envelope level when release started
    trigger_step: Option<usize>, // step when note was triggered (for hold_steps)
    steps_elapsed: usize,        // steps elapsed since trigger
    params: SamplerParams,
}

impl SamplerSynth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            buffer: Vec::new(),
            position: None,
            playback_rate: 1.0,
            envelope: 0.0,
            envelope_phase: EnvelopePhase::Off,
            envelope_samples: 0,
            release_start_level: 0.0,
            trigger_step: None,
            steps_elapsed: 0,
            params: SamplerParams::default(),
        }
    }

    /// Load a sample buffer and associated path
    pub fn set_buffer(&mut self, buffer: Vec<f32>, path: &str) {
        self.buffer = buffer;
        self.params.wav_path = Some(path.to_string());
    }

    fn start_pos_samples(&self) -> f64 {
        self.params.start_point as f64 * self.buffer.len() as f64
    }

    fn end_pos_samples(&self) -> f64 {
        self.params.end_point as f64 * self.buffer.len() as f64
    }

    fn attack_samples(&self) -> f32 {
        self.params.attack * 0.001 * self.sample_rate // ms to samples
    }

    fn decay_samples(&self) -> f32 {
        self.params.decay * 0.001 * self.sample_rate // ms to samples
    }

    fn release_samples(&self) -> f32 {
        self.params.release * 0.001 * self.sample_rate // ms to samples
    }

    fn loop_start_samples(&self) -> f64 {
        self.params.loop_start as f64 * self.buffer.len() as f64
    }

    fn loop_end_samples(&self) -> f64 {
        self.params.loop_end as f64 * self.buffer.len() as f64
    }

    /// Trigger release phase (called by hold_steps countdown or note_off)
    fn start_release(&mut self) {
        if self.envelope_phase != EnvelopePhase::Off && self.envelope_phase != EnvelopePhase::Release {
            self.release_start_level = self.envelope;
            self.envelope_phase = EnvelopePhase::Release;
            self.envelope_samples = 0;
        }
    }
}

impl SoundSource for SamplerSynth {
    fn synth_type(&self) -> SynthType {
        SynthType::Sampler
    }

    fn type_name(&self) -> &'static str {
        "SAMPLER"
    }

    fn default_note(&self) -> u8 {
        60 // C4 = original pitch
    }

    fn trigger(&mut self) {
        self.trigger_with_note(60);
    }

    fn trigger_with_note(&mut self, note: u8) {
        if self.buffer.is_empty() {
            return;
        }
        // Compute playback rate: 2^((note - 60 + pitch_shift) / 12)
        let semitones = (note as f64 - 60.0) + self.params.pitch_shift as f64;
        self.playback_rate = 2.0f64.powf(semitones / 12.0);
        self.position = Some(self.start_pos_samples());
        self.envelope = 0.0;
        self.envelope_samples = 0;
        self.release_start_level = 0.0;
        self.steps_elapsed = 0;
        self.trigger_step = Some(0); // Will be set properly by step_tick
        if self.params.attack > 0.0 {
            self.envelope_phase = EnvelopePhase::Attack;
        } else {
            // Skip attack, go straight to peak (1.0) then decay
            self.envelope = 1.0;
            if self.params.decay > 0.0 {
                self.envelope_phase = EnvelopePhase::Decay;
            } else {
                self.envelope = self.params.sustain;
                self.envelope_phase = EnvelopePhase::Sustain;
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        let Some(pos) = self.position else {
            return 0.0;
        };

        if self.buffer.is_empty() {
            self.position = None;
            return 0.0;
        }

        let end = self.end_pos_samples();

        // Check if we've reached end of playback region
        let new_pos = if pos >= end || pos >= self.buffer.len() as f64 {
            if self.params.loop_enabled && self.envelope_phase != EnvelopePhase::Release {
                // Loop mode: wrap back to loop_start
                let loop_start = self.loop_start_samples();
                let loop_end = self.loop_end_samples().min(self.buffer.len() as f64);
                if loop_end > loop_start {
                    // Wrap within loop region
                    loop_start + ((pos - loop_start) % (loop_end - loop_start))
                } else {
                    // Invalid loop region, stop
                    self.position = None;
                    self.envelope_phase = EnvelopePhase::Off;
                    return 0.0;
                }
            } else {
                // One-shot mode or in release: stop playback
                self.position = None;
                self.envelope_phase = EnvelopePhase::Off;
                return 0.0;
            }
        } else {
            pos
        };

        // Linear interpolation
        let idx = new_pos as usize;
        let frac = (new_pos - idx as f64) as f32;
        let s0 = if idx < self.buffer.len() {
            self.buffer[idx]
        } else {
            0.0
        };
        let s1 = if idx + 1 < self.buffer.len() {
            self.buffer[idx + 1]
        } else {
            s0
        };
        let raw = s0 + (s1 - s0) * frac;

        // Advance position (with loop wrapping)
        let next_pos = new_pos + self.playback_rate;
        if self.params.loop_enabled && self.envelope_phase != EnvelopePhase::Release {
            let loop_start = self.loop_start_samples();
            let loop_end = self.loop_end_samples().min(self.buffer.len() as f64);
            if loop_end > loop_start && next_pos >= loop_end {
                self.position = Some(loop_start + ((next_pos - loop_start) % (loop_end - loop_start)));
            } else {
                self.position = Some(next_pos);
            }
        } else {
            self.position = Some(next_pos);
        }

        // Update envelope
        self.envelope_samples += 1;
        match self.envelope_phase {
            EnvelopePhase::Off => {
                return 0.0;
            }
            EnvelopePhase::Attack => {
                let attack_len = self.attack_samples();
                if attack_len > 0.0 {
                    self.envelope = (self.envelope_samples as f32 / attack_len).min(1.0);
                    if self.envelope >= 1.0 {
                        self.envelope = 1.0;
                        self.envelope_phase = EnvelopePhase::Decay;
                        self.envelope_samples = 0;
                    }
                } else {
                    self.envelope = 1.0;
                    self.envelope_phase = EnvelopePhase::Decay;
                    self.envelope_samples = 0;
                }
            }
            EnvelopePhase::Decay => {
                let decay_len = self.decay_samples();
                let sustain_level = self.params.sustain;
                if decay_len > 0.0 {
                    let progress = (self.envelope_samples as f32 / decay_len).min(1.0);
                    self.envelope = 1.0 - progress * (1.0 - sustain_level);
                    if progress >= 1.0 {
                        self.envelope = sustain_level;
                        self.envelope_phase = EnvelopePhase::Sustain;
                        self.envelope_samples = 0;
                    }
                } else {
                    self.envelope = sustain_level;
                    self.envelope_phase = EnvelopePhase::Sustain;
                    self.envelope_samples = 0;
                }
            }
            EnvelopePhase::Sustain => {
                // Hold at sustain level
                // For one-shot (non-looping), auto-trigger release when near end
                if !self.params.loop_enabled {
                    let end = self.end_pos_samples();
                    let release_time_samples = self.release_samples() as f64 * self.playback_rate;
                    if let Some(p) = self.position {
                        if p + release_time_samples >= end {
                            self.start_release();
                        }
                    }
                }
                // Hold_steps countdown is handled by step_tick()
            }
            EnvelopePhase::Release => {
                let release_len = self.release_samples();
                if release_len > 0.0 {
                    let progress = (self.envelope_samples as f32 / release_len).min(1.0);
                    self.envelope = self.release_start_level * (1.0 - progress);
                    if progress >= 1.0 {
                        self.envelope = 0.0;
                        self.position = None;
                        self.envelope_phase = EnvelopePhase::Off;
                        return 0.0;
                    }
                } else {
                    self.envelope = 0.0;
                    self.position = None;
                    self.envelope_phase = EnvelopePhase::Off;
                    return 0.0;
                }
            }
        }

        raw * self.envelope * self.params.amplitude
    }

    fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                key: "amplitude".into(),
                name: "Amplitude".into(),
                min: 0.0,
                max: 1.0,
                default: 0.8,
            },
            ParamDescriptor {
                key: "attack".into(),
                name: "Attack (ms)".into(),
                min: 0.0,
                max: 50.0,
                default: 0.0,
            },
            ParamDescriptor {
                key: "decay".into(),
                name: "Decay (ms)".into(),
                min: 10.0,
                max: 500.0,
                default: 100.0,
            },
            ParamDescriptor {
                key: "sustain".into(),
                name: "Sustain".into(),
                min: 0.0,
                max: 1.0,
                default: 0.8,
            },
            ParamDescriptor {
                key: "release".into(),
                name: "Release (ms)".into(),
                min: 10.0,
                max: 2000.0,
                default: 200.0,
            },
            ParamDescriptor {
                key: "start_point".into(),
                name: "Start Point".into(),
                min: 0.0,
                max: 1.0,
                default: 0.0,
            },
            ParamDescriptor {
                key: "end_point".into(),
                name: "End Point".into(),
                min: 0.0,
                max: 1.0,
                default: 1.0,
            },
            ParamDescriptor {
                key: "pitch_shift".into(),
                name: "Pitch Shift".into(),
                min: -24.0,
                max: 24.0,
                default: 0.0,
            },
            ParamDescriptor {
                key: "loop_enabled".into(),
                name: "Loop".into(),
                min: 0.0,
                max: 1.0,
                default: 0.0,
            },
            ParamDescriptor {
                key: "loop_start".into(),
                name: "Loop Start".into(),
                min: 0.0,
                max: 1.0,
                default: 0.0,
            },
            ParamDescriptor {
                key: "loop_end".into(),
                name: "Loop End".into(),
                min: 0.0,
                max: 1.0,
                default: 1.0,
            },
            ParamDescriptor {
                key: "hold_steps".into(),
                name: "Hold Steps".into(),
                min: 1.0,
                max: 16.0,
                default: 4.0,
            },
        ]
    }

    fn get_param(&self, key: &str) -> Option<f32> {
        match key {
            "amplitude" => Some(self.params.amplitude),
            "attack" => Some(self.params.attack),
            "decay" => Some(self.params.decay),
            "sustain" => Some(self.params.sustain),
            "release" => Some(self.params.release),
            "start_point" => Some(self.params.start_point),
            "end_point" => Some(self.params.end_point),
            "pitch_shift" => Some(self.params.pitch_shift),
            "loop_enabled" => Some(if self.params.loop_enabled { 1.0 } else { 0.0 }),
            "loop_start" => Some(self.params.loop_start),
            "loop_end" => Some(self.params.loop_end),
            "hold_steps" => Some(self.params.hold_steps as f32),
            _ => None,
        }
    }

    fn set_param(&mut self, key: &str, value: f32) -> bool {
        match key {
            "amplitude" => {
                self.params.amplitude = value.clamp(0.0, 1.0);
                true
            }
            "attack" => {
                self.params.attack = value.clamp(0.0, 50.0);
                true
            }
            "decay" => {
                self.params.decay = value.clamp(10.0, 500.0);
                true
            }
            "sustain" => {
                self.params.sustain = value.clamp(0.0, 1.0);
                true
            }
            "release" => {
                self.params.release = value.clamp(10.0, 2000.0);
                true
            }
            "start_point" => {
                self.params.start_point = value.clamp(0.0, 1.0);
                true
            }
            "end_point" => {
                self.params.end_point = value.clamp(0.0, 1.0);
                true
            }
            "pitch_shift" => {
                self.params.pitch_shift = value.clamp(-24.0, 24.0);
                true
            }
            "loop_enabled" => {
                self.params.loop_enabled = value >= 0.5;
                true
            }
            "loop_start" => {
                self.params.loop_start = value.clamp(0.0, 1.0);
                true
            }
            "loop_end" => {
                self.params.loop_end = value.clamp(0.0, 1.0);
                true
            }
            "hold_steps" => {
                self.params.hold_steps = (value.clamp(1.0, 16.0) as u8).max(1);
                true
            }
            _ => false,
        }
    }

    fn serialize_params(&self) -> Value {
        serde_json::to_value(&self.params).unwrap_or(Value::Null)
    }

    fn deserialize_params(&mut self, params: &Value) {
        if let Ok(p) = serde_json::from_value::<SamplerParams>(params.clone()) {
            self.params = p;
        }
    }

    fn load_buffer(&mut self, buffer: Vec<f32>, path: &str) {
        self.set_buffer(buffer, path);
    }

    fn step_tick(&mut self) {
        // Only count steps if we're playing and in attack/decay/sustain phase
        if self.position.is_some()
            && self.envelope_phase != EnvelopePhase::Off
            && self.envelope_phase != EnvelopePhase::Release
        {
            self.steps_elapsed += 1;
            // Check hold_steps countdown
            if self.steps_elapsed >= self.params.hold_steps as usize {
                self.start_release();
            }
        }
    }

    fn stop(&mut self) {
        self.position = None;
        self.envelope = 0.0;
        self.envelope_phase = EnvelopePhase::Off;
        self.envelope_samples = 0;
    }
}

/// Load a WAV file and return mono f32 samples at the target sample rate
pub fn load_wav(path: &Path, target_sr: f32) -> Result<Vec<f32>> {
    let reader = hound::WavReader::open(path)
        .with_context(|| format!("Failed to open WAV: {}", path.display()))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;
    let wav_sr = spec.sample_rate as f32;

    // Read all samples and convert to mono f32
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    if samples.is_empty() {
        bail!("WAV file is empty: {}", path.display());
    }

    // Convert to mono (average channels)
    let mono: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample if needed (simple linear interpolation)
    if (wav_sr - target_sr).abs() > 1.0 {
        let ratio = wav_sr as f64 / target_sr as f64;
        let new_len = (mono.len() as f64 / ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);
        for i in 0..new_len {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let s0 = mono.get(idx).copied().unwrap_or(0.0);
            let s1 = mono.get(idx + 1).copied().unwrap_or(s0);
            resampled.push(s0 + (s1 - s0) * frac);
        }
        Ok(resampled)
    } else {
        Ok(mono)
    }
}
