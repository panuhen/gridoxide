use super::params::KickParams;

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

        // Pitch envelope: exponential decay from pitch_start to pitch_end
        let freq = self.params.pitch_end
            + (self.params.pitch_start - self.params.pitch_end)
                * (-t * self.params.pitch_decay).exp();

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

        sample
    }
}
