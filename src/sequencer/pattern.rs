use serde::{Deserialize, Serialize};

pub const STEPS: usize = 16;
pub const DEFAULT_TRACKS: usize = 4;
pub const NUM_PATTERNS: usize = 16;
pub const MAX_ARRANGEMENT_ENTRIES: usize = 64;

/// Default MIDI notes for the 4 built-in tracks
pub const DEFAULT_NOTES: [u8; 4] = [
    36, // Kick: C2
    50, // Snare: D3
    60, // HiHat: C4
    33, // Bass: A1 (55 Hz)
];

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
        Self::new_with_tracks(DEFAULT_TRACKS)
    }

    pub fn new_with_tracks(num_tracks: usize) -> Self {
        Self {
            patterns: (0..NUM_PATTERNS).map(|_| Pattern::new_with_tracks(num_tracks)).collect(),
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
        let pat = &self.patterns[index];
        for track in 0..pat.num_tracks() {
            for step in 0..STEPS {
                if pat.get(track, step) {
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
    /// steps[track][step] - dynamic number of tracks
    pub steps: Vec<[StepData; STEPS]>,
}

impl Pattern {
    pub fn new() -> Self {
        Self::new_with_tracks(DEFAULT_TRACKS)
    }

    pub fn new_with_tracks(num_tracks: usize) -> Self {
        let mut steps = Vec::with_capacity(num_tracks);
        for track in 0..num_tracks {
            let default_note = if track < DEFAULT_NOTES.len() {
                DEFAULT_NOTES[track]
            } else {
                60 // C4 for any extra tracks
            };
            steps.push([StepData::off(default_note); STEPS]);
        }
        Self { steps }
    }

    /// Create a pattern with specific default notes per track
    pub fn new_with_notes(default_notes: &[u8]) -> Self {
        let mut steps = Vec::with_capacity(default_notes.len());
        for &note in default_notes {
            steps.push([StepData::off(note); STEPS]);
        }
        Self { steps }
    }

    /// Number of tracks in this pattern
    pub fn num_tracks(&self) -> usize {
        self.steps.len()
    }

    /// Add a new track with the given default note
    pub fn add_track(&mut self, default_note: u8) {
        self.steps.push([StepData::off(default_note); STEPS]);
    }

    /// Remove the last track (if more than 1 remain)
    pub fn remove_track(&mut self, index: usize) {
        if self.steps.len() > 1 && index < self.steps.len() {
            self.steps.remove(index);
        }
    }

    /// Toggle step active state. When activating, uses the step's existing note.
    pub fn toggle(&mut self, track: usize, step: usize) -> bool {
        if track < self.steps.len() && step < STEPS {
            self.steps[track][step].active = !self.steps[track][step].active;
            self.steps[track][step].active
        } else {
            false
        }
    }

    pub fn set(&mut self, track: usize, step: usize, value: bool) {
        if track < self.steps.len() && step < STEPS {
            self.steps[track][step].active = value;
        }
    }

    /// Backward-compatible: returns whether a step is active
    pub fn get(&self, track: usize, step: usize) -> bool {
        if track < self.steps.len() && step < STEPS {
            self.steps[track][step].active
        } else {
            false
        }
    }

    /// Get full step data (active + note)
    pub fn get_step(&self, track: usize, step: usize) -> StepData {
        if track < self.steps.len() && step < STEPS {
            self.steps[track][step]
        } else {
            StepData::off(60)
        }
    }

    /// Set the MIDI note for a step
    pub fn set_note(&mut self, track: usize, step: usize, note: u8) {
        if track < self.steps.len() && step < STEPS {
            self.steps[track][step].note = note.min(127);
        }
    }

    pub fn clear_track(&mut self, track: usize) {
        if track < self.steps.len() {
            let default_note = self.default_note_for_track(track);
            for step in 0..STEPS {
                self.steps[track][step] = StepData::off(default_note);
            }
        }
    }

    pub fn fill_track(&mut self, track: usize) {
        if track < self.steps.len() {
            let default_note = self.default_note_for_track(track);
            for step in 0..STEPS {
                self.steps[track][step] = StepData::on(default_note);
            }
        }
    }

    pub fn clear_all(&mut self) {
        for track in 0..self.steps.len() {
            self.clear_track(track);
        }
    }

    /// Get the default note for a track (from first step or DEFAULT_NOTES)
    fn default_note_for_track(&self, track: usize) -> u8 {
        if track < DEFAULT_NOTES.len() {
            DEFAULT_NOTES[track]
        } else {
            60 // C4
        }
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}
