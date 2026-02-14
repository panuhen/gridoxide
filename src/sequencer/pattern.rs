use serde::{Deserialize, Serialize};

pub const STEPS: usize = 16;
pub const TRACKS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Kick = 0,
    Snare = 1,
    HiHat = 2,
    Bass = 3,
}

impl TrackType {
    pub fn name(&self) -> &'static str {
        match self {
            TrackType::Kick => "KICK",
            TrackType::Snare => "SNARE",
            TrackType::HiHat => "HIHAT",
            TrackType::Bass => "BASS",
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(TrackType::Kick),
            1 => Some(TrackType::Snare),
            2 => Some(TrackType::HiHat),
            3 => Some(TrackType::Bass),
            _ => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// steps[track][step] = active
    pub steps: [[bool; STEPS]; TRACKS],
}

impl Pattern {
    pub fn new() -> Self {
        Self {
            steps: [[false; STEPS]; TRACKS],
        }
    }

    pub fn toggle(&mut self, track: usize, step: usize) -> bool {
        if track < TRACKS && step < STEPS {
            self.steps[track][step] = !self.steps[track][step];
            self.steps[track][step]
        } else {
            false
        }
    }

    pub fn set(&mut self, track: usize, step: usize, value: bool) {
        if track < TRACKS && step < STEPS {
            self.steps[track][step] = value;
        }
    }

    pub fn get(&self, track: usize, step: usize) -> bool {
        if track < TRACKS && step < STEPS {
            self.steps[track][step]
        } else {
            false
        }
    }

    pub fn clear_track(&mut self, track: usize) {
        if track < TRACKS {
            self.steps[track] = [false; STEPS];
        }
    }

    pub fn fill_track(&mut self, track: usize) {
        if track < TRACKS {
            self.steps[track] = [true; STEPS];
        }
    }

    pub fn clear_all(&mut self) {
        self.steps = [[false; STEPS]; TRACKS];
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}
