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

/// Pattern variation (A or B)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Variation {
    #[default]
    A,
    B,
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

    /// Returns true if a pattern has any active steps (in either variation)
    pub fn has_content(&self, index: usize) -> bool {
        if index >= NUM_PATTERNS {
            return false;
        }
        let pat = &self.patterns[index];
        for variation in [Variation::A, Variation::B] {
            for track in 0..pat.num_tracks() {
                for step in 0..STEPS {
                    if pat.get_var(track, step, variation) {
                        return true;
                    }
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

fn default_velocity() -> u8 {
    127
}

fn default_probability() -> u8 {
    100
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StepData {
    pub active: bool,
    pub note: u8, // MIDI note 0-127
    #[serde(default = "default_velocity")]
    pub velocity: u8, // 0-127, default 127
    #[serde(default = "default_probability")]
    pub probability: u8, // 0-100%, default 100
}

impl StepData {
    pub fn off(note: u8) -> Self {
        Self {
            active: false,
            note,
            velocity: 127,
            probability: 100,
        }
    }

    pub fn on(note: u8) -> Self {
        Self {
            active: true,
            note,
            velocity: 127,
            probability: 100,
        }
    }

    pub fn with_velocity(note: u8, velocity: u8) -> Self {
        Self {
            active: true,
            note,
            velocity: velocity.min(127),
            probability: 100,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// steps_a[track][step] - variation A (dynamic number of tracks)
    #[serde(alias = "steps")]
    pub steps_a: Vec<[StepData; STEPS]>,
    /// steps_b[track][step] - variation B (dynamic number of tracks)
    #[serde(default)]
    pub steps_b: Vec<[StepData; STEPS]>,
}

impl Pattern {
    pub fn new() -> Self {
        Self::new_with_tracks(DEFAULT_TRACKS)
    }

    pub fn new_with_tracks(num_tracks: usize) -> Self {
        let mut steps_a = Vec::with_capacity(num_tracks);
        let mut steps_b = Vec::with_capacity(num_tracks);
        for track in 0..num_tracks {
            let default_note = if track < DEFAULT_NOTES.len() {
                DEFAULT_NOTES[track]
            } else {
                60 // C4 for any extra tracks
            };
            steps_a.push([StepData::off(default_note); STEPS]);
            steps_b.push([StepData::off(default_note); STEPS]);
        }
        Self { steps_a, steps_b }
    }

    /// Create a pattern with specific default notes per track
    pub fn new_with_notes(default_notes: &[u8]) -> Self {
        let mut steps_a = Vec::with_capacity(default_notes.len());
        let mut steps_b = Vec::with_capacity(default_notes.len());
        for &note in default_notes {
            steps_a.push([StepData::off(note); STEPS]);
            steps_b.push([StepData::off(note); STEPS]);
        }
        Self { steps_a, steps_b }
    }

    /// Ensure steps_b has the same track count as steps_a
    /// (for backward compatibility when loading old projects)
    pub fn ensure_variation_b(&mut self) {
        while self.steps_b.len() < self.steps_a.len() {
            let track = self.steps_b.len();
            let default_note = if track < DEFAULT_NOTES.len() {
                DEFAULT_NOTES[track]
            } else {
                60
            };
            self.steps_b.push([StepData::off(default_note); STEPS]);
        }
    }

    /// Get steps for a specific variation
    pub fn steps(&self, variation: Variation) -> &Vec<[StepData; STEPS]> {
        match variation {
            Variation::A => &self.steps_a,
            Variation::B => &self.steps_b,
        }
    }

    /// Get mutable steps for a specific variation
    pub fn steps_mut(&mut self, variation: Variation) -> &mut Vec<[StepData; STEPS]> {
        match variation {
            Variation::A => &mut self.steps_a,
            Variation::B => &mut self.steps_b,
        }
    }

    /// Number of tracks in this pattern
    pub fn num_tracks(&self) -> usize {
        self.steps_a.len()
    }

    /// Add a new track with the given default note
    pub fn add_track(&mut self, default_note: u8) {
        self.steps_a.push([StepData::off(default_note); STEPS]);
        self.steps_b.push([StepData::off(default_note); STEPS]);
    }

    /// Remove the last track (if more than 1 remain)
    pub fn remove_track(&mut self, index: usize) {
        if self.steps_a.len() > 1 && index < self.steps_a.len() {
            self.steps_a.remove(index);
        }
        if self.steps_b.len() > 1 && index < self.steps_b.len() {
            self.steps_b.remove(index);
        }
    }

    /// Toggle step active state for variation A (default). When activating, uses the step's existing note.
    pub fn toggle(&mut self, track: usize, step: usize) -> bool {
        self.toggle_var(track, step, Variation::A)
    }

    /// Toggle step active state for a specific variation
    pub fn toggle_var(&mut self, track: usize, step: usize, variation: Variation) -> bool {
        let steps = self.steps_mut(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step].active = !steps[track][step].active;
            steps[track][step].active
        } else {
            false
        }
    }

    pub fn set(&mut self, track: usize, step: usize, value: bool) {
        self.set_var(track, step, value, Variation::A)
    }

    pub fn set_var(&mut self, track: usize, step: usize, value: bool, variation: Variation) {
        let steps = self.steps_mut(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step].active = value;
        }
    }

    /// Backward-compatible: returns whether a step is active (variation A)
    pub fn get(&self, track: usize, step: usize) -> bool {
        self.get_var(track, step, Variation::A)
    }

    /// Returns whether a step is active for a specific variation
    pub fn get_var(&self, track: usize, step: usize, variation: Variation) -> bool {
        let steps = self.steps(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step].active
        } else {
            false
        }
    }

    /// Get full step data (active + note) for variation A
    pub fn get_step(&self, track: usize, step: usize) -> StepData {
        self.get_step_var(track, step, Variation::A)
    }

    /// Get full step data for a specific variation
    pub fn get_step_var(&self, track: usize, step: usize, variation: Variation) -> StepData {
        let steps = self.steps(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step]
        } else {
            StepData::off(60)
        }
    }

    /// Set the MIDI note for a step (variation A)
    pub fn set_note(&mut self, track: usize, step: usize, note: u8) {
        self.set_note_var(track, step, note, Variation::A)
    }

    /// Set the MIDI note for a step for a specific variation
    pub fn set_note_var(&mut self, track: usize, step: usize, note: u8, variation: Variation) {
        let steps = self.steps_mut(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step].note = note.min(127);
        }
    }

    /// Set the velocity for a step (0-127, variation A)
    pub fn set_velocity(&mut self, track: usize, step: usize, velocity: u8) {
        self.set_velocity_var(track, step, velocity, Variation::A)
    }

    /// Set the velocity for a step for a specific variation
    pub fn set_velocity_var(&mut self, track: usize, step: usize, velocity: u8, variation: Variation) {
        let steps = self.steps_mut(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step].velocity = velocity.min(127);
        }
    }

    /// Set the probability for a step (0-100%, variation A)
    pub fn set_probability(&mut self, track: usize, step: usize, probability: u8) {
        self.set_probability_var(track, step, probability, Variation::A)
    }

    /// Set the probability for a step for a specific variation
    pub fn set_probability_var(&mut self, track: usize, step: usize, probability: u8, variation: Variation) {
        let steps = self.steps_mut(variation);
        if track < steps.len() && step < STEPS {
            steps[track][step].probability = probability.min(100);
        }
    }

    /// Clear a track (variation A)
    pub fn clear_track(&mut self, track: usize) {
        self.clear_track_var(track, Variation::A)
    }

    /// Clear a track for a specific variation
    pub fn clear_track_var(&mut self, track: usize, variation: Variation) {
        let default_note = self.default_note_for_track(track);
        let steps = self.steps_mut(variation);
        if track < steps.len() {
            for step in 0..STEPS {
                steps[track][step] = StepData::off(default_note);
            }
        }
    }

    /// Fill a track (variation A)
    pub fn fill_track(&mut self, track: usize) {
        self.fill_track_var(track, Variation::A)
    }

    /// Fill a track for a specific variation
    pub fn fill_track_var(&mut self, track: usize, variation: Variation) {
        let default_note = self.default_note_for_track(track);
        let steps = self.steps_mut(variation);
        if track < steps.len() {
            for step in 0..STEPS {
                steps[track][step] = StepData::on(default_note);
            }
        }
    }

    /// Clear all tracks (variation A)
    pub fn clear_all(&mut self) {
        self.clear_all_var(Variation::A)
    }

    /// Clear all tracks for a specific variation
    pub fn clear_all_var(&mut self, variation: Variation) {
        let num_tracks = self.num_tracks();
        for track in 0..num_tracks {
            self.clear_track_var(track, variation);
        }
    }

    /// Copy variation A to B or B to A
    pub fn copy_variation(&mut self, from: Variation, to: Variation) {
        match (from, to) {
            (Variation::A, Variation::B) => {
                self.steps_b = self.steps_a.clone();
            }
            (Variation::B, Variation::A) => {
                self.steps_a = self.steps_b.clone();
            }
            _ => {} // Same variation, no-op
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
