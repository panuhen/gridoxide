use serde::{Deserialize, Serialize};

/// Filter type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
}

impl FilterType {
    pub fn name(&self) -> &'static str {
        match self {
            FilterType::LowPass => "LP",
            FilterType::HighPass => "HP",
            FilterType::BandPass => "BP",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i % 3 {
            0 => FilterType::LowPass,
            1 => FilterType::HighPass,
            2 => FilterType::BandPass,
            _ => unreachable!(),
        }
    }

    pub fn index(self) -> usize {
        match self {
            FilterType::LowPass => 0,
            FilterType::HighPass => 1,
            FilterType::BandPass => 2,
        }
    }
}

/// State Variable Filter (2-pole SVF)
pub struct SvfFilter {
    sample_rate: f32,
    filter_type: FilterType,
    cutoff: f32,
    resonance: f32,
    // Integrator states
    low: f32,
    band: f32,
    // Precomputed coefficients
    g: f32, // frequency coefficient
    k: f32, // damping coefficient
}

impl SvfFilter {
    pub fn new(sample_rate: f32) -> Self {
        let mut f = Self {
            sample_rate,
            filter_type: FilterType::LowPass,
            cutoff: 2000.0,
            resonance: 0.0,
            low: 0.0,
            band: 0.0,
            g: 0.0,
            k: 0.0,
        };
        f.update_coefficients();
        f
    }

    fn update_coefficients(&mut self) {
        // g = tan(pi * cutoff / sample_rate)
        let freq = self.cutoff.clamp(20.0, self.sample_rate * 0.49);
        self.g = (std::f32::consts::PI * freq / self.sample_rate).tan();
        // k = 2 - 2*resonance (resonance 0..0.95 -> k 2..0.1)
        self.k = 2.0 - 2.0 * self.resonance.clamp(0.0, 0.95);
    }

    pub fn set_cutoff(&mut self, hz: f32) {
        self.cutoff = hz.clamp(20.0, 20000.0);
        self.update_coefficients();
    }

    pub fn set_resonance(&mut self, q: f32) {
        self.resonance = q.clamp(0.0, 0.95);
        self.update_coefficients();
    }

    pub fn set_filter_type(&mut self, ft: FilterType) {
        self.filter_type = ft;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // Trapezoidal SVF
        let a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        let a2 = self.g * a1;
        let a3 = self.g * a2;

        let v3 = input - self.low - self.k * self.band;
        let v1 = a1 * self.band + a2 * v3;
        let v2 = self.low + a2 * self.band + a3 * v3;

        self.band = 2.0 * v1 - self.band;
        self.low = 2.0 * v2 - self.low;

        match self.filter_type {
            FilterType::LowPass => v2,
            FilterType::HighPass => input - self.k * v1 - v2,
            FilterType::BandPass => v1,
        }
    }
}
