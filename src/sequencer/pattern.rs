use serde::{Deserialize, Serialize};

use crate::synth::DEFAULT_NOTES;

pub const STEPS: usize = 16;
pub const TRACKS: usize = 4;
pub const NUM_PATTERNS: usize = 16;
pub const MAX_ARRANGEMENT_ENTRIES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackMode {
    Pattern,
    Song,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ArrangementEntry {
    pub pattern: usize, // 0-15
    pub repeats: usize, // 1-16
}

impl ArrangementEntry {
    pub fn new(pattern: usize, repeats: usize) -> Self {
        Self {
            pattern: pattern.min(NUM_PATTERNS - 1),
            repeats: repeats.clamp(1, 16),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arrangement {
    pub entries: Vec<ArrangementEntry>,
}

impl Arrangement {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(MAX_ARRANGEMENT_ENTRIES),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn append(&mut self, pattern: usize, repeats: usize) {
        if self.entries.len() < MAX_ARRANGEMENT_ENTRIES {
            self.entries.push(ArrangementEntry::new(pattern, repeats));
        }
    }

    pub fn insert(&mut self, position: usize, pattern: usize, repeats: usize) {
        if self.entries.len() < MAX_ARRANGEMENT_ENTRIES && position <= self.entries.len() {
            self.entries
                .insert(position, ArrangementEntry::new(pattern, repeats));
        }
    }

    pub fn remove(&mut self, position: usize) {
        if position < self.entries.len() {
            self.entries.remove(position);
        }
    }

    pub fn set_entry(&mut self, position: usize, pattern: usize, repeats: usize) {
        if position < self.entries.len() {
            self.entries[position] = ArrangementEntry::new(pattern, repeats);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for Arrangement {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternBank {
    pub patterns: Vec<Pattern>, // always NUM_PATTERNS length
}

impl PatternBank {
    pub fn new() -> Self {
        Self {
            patterns: (0..NUM_PATTERNS).map(|_| Pattern::new()).collect(),
        }
    }

    pub fn get(&self, index: usize) -> &Pattern {
        &self.patterns[index.min(NUM_PATTERNS - 1)]
    }

    pub fn get_mut(&mut self, index: usize) -> &mut Pattern {
        &mut self.patterns[index.min(NUM_PATTERNS - 1)]
    }

    /// Returns true if a pattern has any active steps
    pub fn has_content(&self, index: usize) -> bool {
        if index >= NUM_PATTERNS {
            return false;
        }
        for track in 0..TRACKS {
            for step in 0..STEPS {
                if self.patterns[index].get(track, step) {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for PatternBank {
    fn default() -> Self {
        Self::new()
    }
}

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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StepData {
    pub active: bool,
    pub note: u8, // MIDI note 0-127
}

impl StepData {
    pub fn off(note: u8) -> Self {
        Self {
            active: false,
            note,
        }
    }

    pub fn on(note: u8) -> Self {
        Self {
            active: true,
            note,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// steps[track][step]
    pub steps: [[StepData; STEPS]; TRACKS],
}

impl Pattern {
    pub fn new() -> Self {
        let mut steps = [[StepData::off(60); STEPS]; TRACKS];
        // Initialize each track with its default note
        for track in 0..TRACKS {
            let default_note = DEFAULT_NOTES[track];
            for step in 0..STEPS {
                steps[track][step] = StepData::off(default_note);
            }
        }
        Self { steps }
    }

    /// Toggle step active state. When activating, uses the step's existing note.
    pub fn toggle(&mut self, track: usize, step: usize) -> bool {
        if track < TRACKS && step < STEPS {
            self.steps[track][step].active = !self.steps[track][step].active;
            self.steps[track][step].active
        } else {
            false
        }
    }

    pub fn set(&mut self, track: usize, step: usize, value: bool) {
        if track < TRACKS && step < STEPS {
            self.steps[track][step].active = value;
        }
    }

    /// Backward-compatible: returns whether a step is active
    pub fn get(&self, track: usize, step: usize) -> bool {
        if track < TRACKS && step < STEPS {
            self.steps[track][step].active
        } else {
            false
        }
    }

    /// Get full step data (active + note)
    pub fn get_step(&self, track: usize, step: usize) -> StepData {
        if track < TRACKS && step < STEPS {
            self.steps[track][step]
        } else {
            StepData::off(60)
        }
    }

    /// Set the MIDI note for a step
    pub fn set_note(&mut self, track: usize, step: usize, note: u8) {
        if track < TRACKS && step < STEPS {
            self.steps[track][step].note = note.min(127);
        }
    }

    pub fn clear_track(&mut self, track: usize) {
        if track < TRACKS {
            let default_note = DEFAULT_NOTES[track];
            for step in 0..STEPS {
                self.steps[track][step] = StepData::off(default_note);
            }
        }
    }

    pub fn fill_track(&mut self, track: usize) {
        if track < TRACKS {
            let default_note = DEFAULT_NOTES[track];
            for step in 0..STEPS {
                self.steps[track][step] = StepData::on(default_note);
            }
        }
    }

    pub fn clear_all(&mut self) {
        for track in 0..TRACKS {
            self.clear_track(track);
        }
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}
