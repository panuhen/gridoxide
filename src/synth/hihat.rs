use super::params::{midi_to_freq, HiHatParams, DEFAULT_NOTES};

/// Hi-hat synthesizer
/// High-passed noise with very short envelope
pub struct HiHatSynth {
    phase: Option<usize>,
    sample_rate: f32,
    duration_samples: usize,
    noise_state: u32,
    filter_state: f32,
    params: HiHatParams,
    /// Brightness ratio from note (1.0 = default)
    brightness_ratio: f32,
}

impl HiHatSynth {
    pub fn new(sample_rate: f32) -> Self {
        let params = HiHatParams::default();
        Self {
            phase: None,
            sample_rate,
            duration_samples: (sample_rate * 0.05) as usize,
            noise_state: 67890,
            filter_state: 0.0,
            params,
            brightness_ratio: 1.0,
        }
    }

    /// Update parameters
    pub fn set_params(&mut self, params: HiHatParams) {
        self.params = params;
        // Adjust duration based on open parameter
        let base_duration = if self.params.open > 0.5 { 0.2 } else { 0.05 };
        let open_factor = 1.0 + self.params.open * 3.0;
        self.duration_samples = (self.sample_rate * base_duration * open_factor) as usize;
    }

    /// Get current parameters
    pub fn params(&self) -> &HiHatParams {
        &self.params
    }

    pub fn trigger(&mut self) {
        self.phase = Some(0);
        self.filter_state = 0.0;
        self.brightness_ratio = 1.0;
        // Recalculate duration on trigger based on open parameter
        let base_duration = if self.params.open > 0.5 { 0.2 } else { 0.05 };
        let open_factor = 1.0 + self.params.open * 3.0;
        self.duration_samples = (self.sample_rate * base_duration * open_factor) as usize;
    }

    /// Trigger with a specific MIDI note (scales brightness)
    pub fn trigger_with_note(&mut self, note: u8) {
        self.phase = Some(0);
        self.filter_state = 0.0;
        self.brightness_ratio = midi_to_freq(note) / midi_to_freq(DEFAULT_NOTES[2]);
        // Recalculate duration on trigger based on open parameter
        let base_duration = if self.params.open > 0.5 { 0.2 } else { 0.05 };
        let open_factor = 1.0 + self.params.open * 3.0;
        self.duration_samples = (self.sample_rate * base_duration * open_factor) as usize;
    }

    /// Simple linear congruential generator for noise
    fn next_noise(&mut self) -> f32 {
        self.noise_state = self.noise_state.wrapping_mul(1103515245).wrapping_add(12345);
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

        // Generate noise
        let noise = self.next_noise();

        // High-pass filter (adjustable with tone parameter)
        // Higher tone = more high frequencies (brighter)
        // Scale alpha by brightness_ratio: higher notes = brighter
        let base_alpha = 0.9 + self.params.tone * 0.09; // 0.9 to 0.99
        let alpha = (base_alpha * self.brightness_ratio).clamp(0.5, 0.999);
        let filtered = noise - self.filter_state + alpha * self.filter_state;
        self.filter_state = noise;

        // Amplitude envelope - decay controlled by params
        // Open hi-hat has slower decay
        let effective_decay = self.params.decay * (1.0 - self.params.open * 0.7);
        let amp = (-t * effective_decay).exp();

        // Advance phase
        self.phase = Some(phase + 1);

        filtered * amp * 0.4
    }
}
