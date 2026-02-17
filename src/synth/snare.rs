use serde_json::Value;

use super::params::{midi_to_freq, SnareParams, DEFAULT_NOTES};
use super::source::{ParamDescriptor, SoundSource, SynthType};

/// Snare drum synthesizer
/// Mix of noise burst and body tone with fast decay
pub struct SnareSynth {
    phase: Option<usize>,
    sample_rate: f32,
    duration_samples: usize,
    noise_state: u32,
    tone_phase: f32,
    params: SnareParams,
    /// Tone frequency ratio from note (1.0 = default)
    tone_ratio: f32,
    /// Velocity scale (0.0-1.0) for amplitude
    velocity_scale: f32,
}

impl SnareSynth {
    pub fn new(sample_rate: f32) -> Self {
        let params = SnareParams::default();
        Self {
            phase: None,
            sample_rate,
            duration_samples: (sample_rate * 0.15) as usize,
            noise_state: 12345,
            tone_phase: 0.0,
            params,
            tone_ratio: 1.0,
            velocity_scale: 1.0,
        }
    }

    /// Update parameters
    pub fn set_params(&mut self, params: SnareParams) {
        self.params = params;
    }

    /// Get current parameters
    pub fn params(&self) -> &SnareParams {
        &self.params
    }

    pub fn trigger(&mut self) {
        self.phase = Some(0);
        self.tone_phase = 0.0;
        self.tone_ratio = 1.0;
    }

    /// Trigger with a specific MIDI note (scales tone frequency)
    pub fn trigger_with_note(&mut self, note: u8) {
        self.phase = Some(0);
        self.tone_phase = 0.0;
        self.tone_ratio = midi_to_freq(note) / midi_to_freq(DEFAULT_NOTES[1]);
    }

    /// Set velocity scale from MIDI velocity (0-127)
    pub fn set_velocity(&mut self, velocity: u8) {
        self.velocity_scale = velocity as f32 / 127.0;
    }

    /// Simple linear congruential generator for noise
    fn next_noise(&mut self) -> f32 {
        self.noise_state = self.noise_state.wrapping_mul(1103515245).wrapping_add(12345);
        // Convert to -1.0 to 1.0 range
        (self.noise_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn next_sample(&mut self) -> f32 {
        let Some(phase) = self.phase else {
            return 0.0;
        };

        if phase >= self.duration_samples {
            self.phase = None;
            return 0.0;
        }

        let t = phase as f32 / self.sample_rate;

        // Noise component with fast decay
        let noise_amp = (-t * self.params.noise_decay).exp();
        let mut noise = self.next_noise() * noise_amp;

        // Apply snappy (high-freq emphasis) - simple high-pass effect
        if self.params.snappy > 0.0 {
            noise = noise * (1.0 + self.params.snappy * 2.0);
        }

        // Body tone with medium decay, scaled by tone_ratio
        let tone_amp = (-t * self.params.tone_decay).exp();
        self.tone_phase += (self.params.tone_freq * self.tone_ratio) / self.sample_rate;
        if self.tone_phase >= 1.0 {
            self.tone_phase -= 1.0;
        }
        let tone = (self.tone_phase * std::f32::consts::TAU).sin() * tone_amp;

        // Advance phase
        self.phase = Some(phase + 1);

        // Mix noise and tone based on tone_mix parameter
        let noise_level = 1.0 - self.params.tone_mix;
        let tone_level = self.params.tone_mix;

        // Apply velocity scaling
        (noise * noise_level * 0.6 + tone * tone_level * 0.5) * 0.7 * self.velocity_scale
    }
}

impl SoundSource for SnareSynth {
    fn synth_type(&self) -> SynthType { SynthType::Snare }
    fn type_name(&self) -> &'static str { "SNARE" }
    fn default_note(&self) -> u8 { DEFAULT_NOTES[1] }
    fn trigger(&mut self) { self.trigger(); }
    fn trigger_with_note(&mut self, note: u8) { self.trigger_with_note(note); }
    fn set_velocity_scale(&mut self, velocity: u8) { self.set_velocity(velocity); }
    fn next_sample(&mut self) -> f32 { self.next_sample() }

    fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor { key: "tone_freq".into(), name: "Tone Freq".into(), min: 120.0, max: 300.0, default: 180.0 },
            ParamDescriptor { key: "tone_decay".into(), name: "Tone Decay".into(), min: 10.0, max: 40.0, default: 20.0 },
            ParamDescriptor { key: "noise_decay".into(), name: "Noise Decay".into(), min: 8.0, max: 30.0, default: 15.0 },
            ParamDescriptor { key: "tone_mix".into(), name: "Tone Mix".into(), min: 0.0, max: 1.0, default: 0.4 },
            ParamDescriptor { key: "snappy".into(), name: "Snappy".into(), min: 0.0, max: 1.0, default: 0.6 },
        ]
    }

    fn get_param(&self, key: &str) -> Option<f32> {
        match key {
            "tone_freq" => Some(self.params.tone_freq),
            "tone_decay" => Some(self.params.tone_decay),
            "noise_decay" => Some(self.params.noise_decay),
            "tone_mix" => Some(self.params.tone_mix),
            "snappy" => Some(self.params.snappy),
            _ => None,
        }
    }

    fn set_param(&mut self, key: &str, value: f32) -> bool {
        match key {
            "tone_freq" => { self.params.tone_freq = value; true }
            "tone_decay" => { self.params.tone_decay = value; true }
            "noise_decay" => { self.params.noise_decay = value; true }
            "tone_mix" => { self.params.tone_mix = value; true }
            "snappy" => { self.params.snappy = value; true }
            _ => false,
        }
    }

    fn serialize_params(&self) -> Value {
        serde_json::to_value(&self.params).unwrap_or(Value::Null)
    }

    fn deserialize_params(&mut self, params: &Value) {
        if let Ok(p) = serde_json::from_value::<SnareParams>(params.clone()) {
            self.set_params(p);
        }
    }
}
