use serde_json::Value;

use super::params::{midi_to_freq, KickParams, DEFAULT_NOTES};
use super::source::{ParamDescriptor, SoundSource, SynthType};

/// Kick drum synthesizer state
pub struct KickSynth {
    /// Current sample index (None = not playing)
    sample_index: Option<usize>,
    /// Sample rate
    sample_rate: f32,
    /// Total duration in samples
    duration_samples: usize,
    /// Accumulated oscillator phase (0.0 to 1.0)
    osc_phase: f32,
    /// Synth parameters
    params: KickParams,
    /// Pitch ratio from note (1.0 = default pitch)
    pitch_ratio: f32,
    /// Velocity scale (0.0-1.0) for amplitude
    velocity_scale: f32,
}

impl KickSynth {
    pub fn new(sample_rate: f32) -> Self {
        let params = KickParams::default();
        // Duration based on amp_decay: longer decay = longer sound
        let duration_samples = (sample_rate * (0.1 + 0.2 * (20.0 - params.amp_decay) / 15.0)) as usize;
        Self {
            sample_index: None,
            sample_rate,
            duration_samples,
            osc_phase: 0.0,
            params,
            pitch_ratio: 1.0,
            velocity_scale: 1.0,
        }
    }

    /// Update parameters
    pub fn set_params(&mut self, params: KickParams) {
        self.params = params;
        // Recalculate duration
        self.duration_samples =
            (self.sample_rate * (0.1 + 0.2 * (20.0 - self.params.amp_decay) / 15.0)) as usize;
    }

    /// Get current parameters
    pub fn params(&self) -> &KickParams {
        &self.params
    }

    /// Trigger the kick drum
    pub fn trigger(&mut self) {
        self.sample_index = Some(0);
        self.osc_phase = 0.0;
        self.pitch_ratio = 1.0;
    }

    /// Trigger with a specific MIDI note (scales pitch envelope)
    pub fn trigger_with_note(&mut self, note: u8) {
        self.sample_index = Some(0);
        self.osc_phase = 0.0;
        self.pitch_ratio = midi_to_freq(note) / midi_to_freq(DEFAULT_NOTES[0]);
    }

    /// Set velocity scale from MIDI velocity (0-127)
    pub fn set_velocity(&mut self, velocity: u8) {
        self.velocity_scale = velocity as f32 / 127.0;
    }

    /// Generate the next sample
    pub fn next_sample(&mut self) -> f32 {
        let Some(index) = self.sample_index else {
            return 0.0;
        };

        if index >= self.duration_samples {
            self.sample_index = None;
            return 0.0;
        }

        let t = index as f32 / self.sample_rate;

        // Pitch envelope: exponential decay from pitch_start to pitch_end, scaled by pitch_ratio
        let freq = (self.params.pitch_end
            + (self.params.pitch_start - self.params.pitch_end)
                * (-t * self.params.pitch_decay).exp())
            * self.pitch_ratio;

        // Accumulate phase incrementally
        self.osc_phase += freq / self.sample_rate;
        if self.osc_phase >= 1.0 {
            self.osc_phase -= 1.0;
        }

        // Oscillator
        let osc = (self.osc_phase * std::f32::consts::TAU).sin();

        // Amplitude envelope
        let amp = (-t * self.params.amp_decay).exp();

        // Attack click
        let click = if t < 0.005 {
            (1.0 - t / 0.005) * self.params.click
        } else {
            0.0
        };

        // Advance sample index
        self.sample_index = Some(index + 1);

        // Mix oscillator and click
        let mut sample = (osc + click * (t * 1000.0).sin()) * amp * 0.7;

        // Drive (soft saturation)
        if self.params.drive > 0.0 {
            let drive_amount = 1.0 + self.params.drive * 4.0;
            sample = (sample * drive_amount).tanh() / drive_amount.tanh();
        }

        // Apply velocity scaling
        sample * self.velocity_scale
    }
}

impl SoundSource for KickSynth {
    fn synth_type(&self) -> SynthType { SynthType::Kick }
    fn type_name(&self) -> &'static str { "KICK" }
    fn default_note(&self) -> u8 { DEFAULT_NOTES[0] }
    fn trigger(&mut self) { self.trigger(); }
    fn trigger_with_note(&mut self, note: u8) { self.trigger_with_note(note); }
    fn set_velocity_scale(&mut self, velocity: u8) { self.set_velocity(velocity); }
    fn next_sample(&mut self) -> f32 { self.next_sample() }

    fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor { key: "pitch_start".into(), name: "Pitch Start".into(), min: 80.0, max: 250.0, default: 150.0 },
            ParamDescriptor { key: "pitch_end".into(), name: "Pitch End".into(), min: 30.0, max: 80.0, default: 50.0 },
            ParamDescriptor { key: "pitch_decay".into(), name: "Pitch Decay".into(), min: 4.0, max: 20.0, default: 8.0 },
            ParamDescriptor { key: "amp_decay".into(), name: "Amp Decay".into(), min: 5.0, max: 20.0, default: 10.0 },
            ParamDescriptor { key: "click".into(), name: "Click".into(), min: 0.0, max: 1.0, default: 0.3 },
            ParamDescriptor { key: "drive".into(), name: "Drive".into(), min: 0.0, max: 1.0, default: 0.0 },
        ]
    }

    fn get_param(&self, key: &str) -> Option<f32> {
        match key {
            "pitch_start" => Some(self.params.pitch_start),
            "pitch_end" => Some(self.params.pitch_end),
            "pitch_decay" => Some(self.params.pitch_decay),
            "amp_decay" => Some(self.params.amp_decay),
            "click" => Some(self.params.click),
            "drive" => Some(self.params.drive),
            _ => None,
        }
    }

    fn set_param(&mut self, key: &str, value: f32) -> bool {
        match key {
            "pitch_start" => { self.params.pitch_start = value; true }
            "pitch_end" => { self.params.pitch_end = value; true }
            "pitch_decay" => { self.params.pitch_decay = value; true }
            "amp_decay" => {
                self.params.amp_decay = value;
                self.duration_samples = (self.sample_rate * (0.1 + 0.2 * (20.0 - self.params.amp_decay) / 15.0)) as usize;
                true
            }
            "click" => { self.params.click = value; true }
            "drive" => { self.params.drive = value; true }
            _ => false,
        }
    }

    fn serialize_params(&self) -> Value {
        serde_json::to_value(&self.params).unwrap_or(Value::Null)
    }

    fn deserialize_params(&mut self, params: &Value) {
        if let Ok(p) = serde_json::from_value::<KickParams>(params.clone()) {
            self.set_params(p);
        }
    }
}
