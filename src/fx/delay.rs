/// Ring buffer delay effect with feedback and mix
pub struct Delay {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    time_ms: f32,
    feedback: f32,
    mix: f32,
    // Smoothed read position to avoid clicks
    current_delay_samples: f32,
    target_delay_samples: f32,
}

impl Delay {
    pub fn new(sample_rate: f32) -> Self {
        // Max 500ms at sample_rate
        let max_samples = (sample_rate * 0.5) as usize + 1;
        Self {
            buffer: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
            time_ms: 200.0,
            feedback: 0.3,
            mix: 0.2,
            current_delay_samples: sample_rate * 0.2,
            target_delay_samples: sample_rate * 0.2,
        }
    }

    pub fn set_time(&mut self, ms: f32) {
        self.time_ms = ms.clamp(10.0, 500.0);
        self.target_delay_samples = self.sample_rate * self.time_ms / 1000.0;
    }

    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.9);
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // Smooth delay time changes to avoid clicks
        let smooth_speed = 0.001;
        self.current_delay_samples += (self.target_delay_samples - self.current_delay_samples) * smooth_speed;

        // Read from buffer with linear interpolation
        let delay_samples = self.current_delay_samples;
        let read_pos_f = self.write_pos as f32 - delay_samples;
        let buf_len = self.buffer.len() as f32;
        let read_pos_f = if read_pos_f < 0.0 {
            read_pos_f + buf_len
        } else {
            read_pos_f
        };

        let read_idx = read_pos_f as usize;
        let frac = read_pos_f - read_idx as f32;
        let idx0 = read_idx % self.buffer.len();
        let idx1 = (read_idx + 1) % self.buffer.len();
        let delayed = self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac;

        // Write input + feedback to buffer
        self.buffer[self.write_pos] = input + delayed * self.feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();

        // Dry/wet mix
        input * (1.0 - self.mix) + delayed * self.mix
    }
}
