/// Schroeder reverb with 4 parallel comb filters and 2 series allpass filters (stereo)
pub struct StereoReverb {
    // Left channel: 4 comb filters + 2 allpass
    comb_l: [CombFilter; 4],
    allpass_l: [AllpassFilter; 2],
    // Right channel: slightly offset delays for stereo spread
    comb_r: [CombFilter; 4],
    allpass_r: [AllpassFilter; 2],
    decay: f32,
    mix: f32,
    damping: f32,
}

impl StereoReverb {
    pub fn new(sample_rate: f32) -> Self {
        // Comb filter delay times in samples (prime-ish numbers for less metallic sound)
        // ~29ms, ~34ms, ~39ms, ~44ms
        let comb_delays_l = [
            (sample_rate * 0.0297) as usize,
            (sample_rate * 0.0341) as usize,
            (sample_rate * 0.0393) as usize,
            (sample_rate * 0.0442) as usize,
        ];
        // Slightly offset for right channel (stereo spread)
        let comb_delays_r = [
            (sample_rate * 0.0307) as usize,
            (sample_rate * 0.0353) as usize,
            (sample_rate * 0.0401) as usize,
            (sample_rate * 0.0457) as usize,
        ];
        // Allpass delays: ~5ms, ~1.7ms
        let allpass_delays_l = [
            (sample_rate * 0.005) as usize,
            (sample_rate * 0.0017) as usize,
        ];
        let allpass_delays_r = [
            (sample_rate * 0.0053) as usize,
            (sample_rate * 0.0019) as usize,
        ];

        let decay = 0.5;
        let damping = 0.5;

        Self {
            comb_l: [
                CombFilter::new(comb_delays_l[0], decay, damping),
                CombFilter::new(comb_delays_l[1], decay, damping),
                CombFilter::new(comb_delays_l[2], decay, damping),
                CombFilter::new(comb_delays_l[3], decay, damping),
            ],
            allpass_l: [
                AllpassFilter::new(allpass_delays_l[0]),
                AllpassFilter::new(allpass_delays_l[1]),
            ],
            comb_r: [
                CombFilter::new(comb_delays_r[0], decay, damping),
                CombFilter::new(comb_delays_r[1], decay, damping),
                CombFilter::new(comb_delays_r[2], decay, damping),
                CombFilter::new(comb_delays_r[3], decay, damping),
            ],
            allpass_r: [
                AllpassFilter::new(allpass_delays_r[0]),
                AllpassFilter::new(allpass_delays_r[1]),
            ],
            decay,
            mix: 0.3,
            damping,
        }
    }

    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay.clamp(0.1, 0.95);
        for c in &mut self.comb_l {
            c.set_feedback(self.decay);
        }
        for c in &mut self.comb_r {
            c.set_feedback(self.decay);
        }
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn set_damping(&mut self, damping: f32) {
        self.damping = damping.clamp(0.0, 1.0);
        for c in &mut self.comb_l {
            c.set_damping(self.damping);
        }
        for c in &mut self.comb_r {
            c.set_damping(self.damping);
        }
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Sum of 4 parallel comb filters per channel
        let mut wet_l = 0.0f32;
        for c in &mut self.comb_l {
            wet_l += c.process(left);
        }
        wet_l *= 0.25; // normalize

        let mut wet_r = 0.0f32;
        for c in &mut self.comb_r {
            wet_r += c.process(right);
        }
        wet_r *= 0.25;

        // Series allpass filters
        for ap in &mut self.allpass_l {
            wet_l = ap.process(wet_l);
        }
        for ap in &mut self.allpass_r {
            wet_r = ap.process(wet_r);
        }

        // Dry/wet mix
        let out_l = left * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = right * (1.0 - self.mix) + wet_r * self.mix;

        (out_l, out_r)
    }
}

/// Comb filter with damping (one-pole LP in feedback path)
struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp_state: f32,
    damping: f32,
}

impl CombFilter {
    fn new(delay: usize, feedback: f32, damping: f32) -> Self {
        Self {
            buffer: vec![0.0; delay.max(1)],
            pos: 0,
            feedback,
            damp_state: 0.0,
            damping,
        }
    }

    fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback;
    }

    fn set_damping(&mut self, damping: f32) {
        self.damping = damping;
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];

        // One-pole LP damping in feedback path
        self.damp_state = delayed * (1.0 - self.damping) + self.damp_state * self.damping;

        self.buffer[self.pos] = input + self.damp_state * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();

        delayed
    }
}

/// Allpass filter for diffusion
struct AllpassFilter {
    buffer: Vec<f32>,
    pos: usize,
}

impl AllpassFilter {
    fn new(delay: usize) -> Self {
        Self {
            buffer: vec![0.0; delay.max(1)],
            pos: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let coeff = 0.5f32;

        let output = -input + delayed;
        self.buffer[self.pos] = input + delayed * coeff;
        self.pos = (self.pos + 1) % self.buffer.len();

        output
    }
}
