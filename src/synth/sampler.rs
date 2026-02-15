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
    pub decay: f32,        // 10-2000 ms, default 500
    pub start_point: f32,  // 0.0-1.0 (fraction of buffer), default 0.0
    pub end_point: f32,    // 0.0-1.0 (fraction of buffer), default 1.0
    pub pitch_shift: f32,  // -24 to +24 semitones, default 0
    #[serde(default)]
    pub wav_path: Option<String>, // for display and serialization
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            amplitude: 0.8,
            attack: 0.0,
            decay: 500.0,
            start_point: 0.0,
            end_point: 1.0,
            pitch_shift: 0.0,
            wav_path: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EnvelopePhase {
    Off,
    Attack,
    Sustain,
    Decay,
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
        if self.params.attack > 0.0 {
            self.envelope_phase = EnvelopePhase::Attack;
        } else {
            self.envelope = 1.0;
            self.envelope_phase = EnvelopePhase::Sustain;
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

        // Check if we've reached end
        if pos >= end || pos >= self.buffer.len() as f64 {
            self.position = None;
            self.envelope_phase = EnvelopePhase::Off;
            return 0.0;
        }

        // Linear interpolation
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
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

        // Advance position
        self.position = Some(pos + self.playback_rate);

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
                        self.envelope_phase = EnvelopePhase::Sustain;
                        self.envelope_samples = 0;
                    }
                } else {
                    self.envelope = 1.0;
                    self.envelope_phase = EnvelopePhase::Sustain;
                    self.envelope_samples = 0;
                }
            }
            EnvelopePhase::Sustain => {
                // Start decay immediately (one-shot samples)
                self.envelope_phase = EnvelopePhase::Decay;
                self.envelope_samples = 0;
            }
            EnvelopePhase::Decay => {
                let decay_len = self.decay_samples();
                if decay_len > 0.0 {
                    self.envelope = 1.0 - (self.envelope_samples as f32 / decay_len).min(1.0);
                    if self.envelope <= 0.0 {
                        self.envelope = 0.0;
                        self.position = None;
                        self.envelope_phase = EnvelopePhase::Off;
                        return 0.0;
                    }
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
                max: 2000.0,
                default: 500.0,
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
        ]
    }

    fn get_param(&self, key: &str) -> Option<f32> {
        match key {
            "amplitude" => Some(self.params.amplitude),
            "attack" => Some(self.params.attack),
            "decay" => Some(self.params.decay),
            "start_point" => Some(self.params.start_point),
            "end_point" => Some(self.params.end_point),
            "pitch_shift" => Some(self.params.pitch_shift),
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
                self.params.decay = value.clamp(10.0, 2000.0);
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
