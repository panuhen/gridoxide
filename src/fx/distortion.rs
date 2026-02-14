/// Tanh soft-clip distortion with dry/wet mix
pub struct Distortion {
    drive: f32,
    mix: f32,
}

impl Distortion {
    pub fn new() -> Self {
        Self {
            drive: 0.1,
            mix: 0.5,
        }
    }

    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(0.0, 1.0);
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn process(&self, input: f32) -> f32 {
        let gain = 1.0 + self.drive * 10.0;
        let norm = gain.tanh();
        let wet = (input * gain).tanh() / norm;
        input * (1.0 - self.mix) + wet * self.mix
    }
}
