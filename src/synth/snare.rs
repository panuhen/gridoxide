use super::params::SnareParams;

/// Snare drum synthesizer
/// Mix of noise burst and body tone with fast decay
pub struct SnareSynth {
    phase: Option<usize>,
    sample_rate: f32,
    duration_samples: usize,
    noise_state: u32,
    tone_phase: f32,
    params: SnareParams,
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

        // Body tone with medium decay
        let tone_amp = (-t * self.params.tone_decay).exp();
        self.tone_phase += self.params.tone_freq / self.sample_rate;
        if self.tone_phase >= 1.0 {
            self.tone_phase -= 1.0;
        }
        let tone = (self.tone_phase * std::f32::consts::TAU).sin() * tone_amp;

        // Advance phase
        self.phase = Some(phase + 1);

        // Mix noise and tone based on tone_mix parameter
        let noise_level = 1.0 - self.params.tone_mix;
        let tone_level = self.params.tone_mix;

        (noise * noise_level * 0.6 + tone * tone_level * 0.5) * 0.7
    }
}
