use super::pattern::STEPS;

/// BPM timing - generates step ticks on the audio thread
pub struct Clock {
    bpm: f32,
    sample_rate: f32,
    samples_per_step: f32,
    sample_counter: f32,
    current_step: usize,
    playing: bool,
}

impl Clock {
    pub fn new(sample_rate: f32, bpm: f32) -> Self {
        let mut clock = Self {
            bpm,
            sample_rate,
            samples_per_step: 0.0,
            sample_counter: 0.0,
            current_step: 0,
            playing: false,
        };
        clock.recalculate_timing();
        clock
    }

    fn recalculate_timing(&mut self) {
        // 16th notes at given BPM
        // 1 beat = 4 16th notes
        // samples_per_beat = sample_rate * 60 / bpm
        // samples_per_step = samples_per_beat / 4
        let samples_per_beat = self.sample_rate * 60.0 / self.bpm;
        self.samples_per_step = samples_per_beat / 4.0;
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(60.0, 200.0);
        self.recalculate_timing();
    }

    pub fn current_step(&self) -> usize {
        self.current_step
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Called once per sample. Returns Some(step) when a new step is triggered.
    pub fn tick(&mut self) -> Option<usize> {
        if !self.playing {
            return None;
        }

        self.sample_counter += 1.0;
        if self.sample_counter >= self.samples_per_step {
            self.sample_counter -= self.samples_per_step;
            let step = self.current_step;
            self.current_step = (self.current_step + 1) % STEPS;
            return Some(step);
        }
        None
    }

    pub fn play(&mut self) {
        if !self.playing {
            self.playing = true;
            // Trigger step 0 immediately when starting
            self.sample_counter = self.samples_per_step;
        }
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.current_step = 0;
        self.sample_counter = 0.0;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }
}
