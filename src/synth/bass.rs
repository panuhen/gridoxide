use super::params::BassParams;

/// Bass synthesizer
/// Simple sine/saw at low frequency with sustain
pub struct BassSynth {
    phase: Option<usize>,
    sample_rate: f32,
    duration_samples: usize,
    osc_phase: f32,
    sub_phase: f32,
    params: BassParams,
}

impl BassSynth {
    pub fn new(sample_rate: f32) -> Self {
        let params = BassParams::default();
        Self {
            phase: None,
            sample_rate,
            duration_samples: (sample_rate * 0.25) as usize,
            osc_phase: 0.0,
            sub_phase: 0.0,
            params,
        }
    }

    /// Update parameters
    pub fn set_params(&mut self, params: BassParams) {
        self.params = params;
    }

    /// Get current parameters
    pub fn params(&self) -> &BassParams {
        &self.params
    }

    pub fn trigger(&mut self) {
        self.phase = Some(0);
        self.osc_phase = 0.0;
        self.sub_phase = 0.0;
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

        // Main oscillator phase
        self.osc_phase += self.params.frequency / self.sample_rate;
        if self.osc_phase >= 1.0 {
            self.osc_phase -= 1.0;
        }

        // Sub oscillator phase (one octave down)
        self.sub_phase += (self.params.frequency * 0.5) / self.sample_rate;
        if self.sub_phase >= 1.0 {
            self.sub_phase -= 1.0;
        }

        // Sine wave
        let sine = (self.osc_phase * std::f32::consts::TAU).sin();

        // Saw wave for harmonics
        let saw = self.osc_phase * 2.0 - 1.0;

        // Sub oscillator (sine, one octave down)
        let sub = (self.sub_phase * std::f32::consts::TAU).sin();

        // Mix based on saw_mix parameter
        let main_osc = sine * (1.0 - self.params.saw_mix) + saw * self.params.saw_mix;

        // Add sub
        let osc = main_osc * (1.0 - self.params.sub * 0.5) + sub * self.params.sub * 0.5;

        // Amplitude envelope: quick attack, parameterized decay
        let attack = 0.01;
        let amp = if t < attack {
            t / attack
        } else {
            (-(t - attack) * self.params.decay).exp()
        };

        // Advance phase
        self.phase = Some(phase + 1);

        osc * amp * 0.6
    }
}
