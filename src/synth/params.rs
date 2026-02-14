use serde::{Deserialize, Serialize};

/// Convert MIDI note number to frequency in Hz
/// A4 (69) = 440 Hz
pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

/// Note name from MIDI note number (e.g., 60 -> "C4", 61 -> "C#4")
pub fn note_name(note: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (note / 12) as i32 - 1;
    format!("{}{}", names[note as usize % 12], octave)
}

/// Default MIDI notes per track (produce same sound as current defaults)
pub const DEFAULT_NOTES: [u8; 4] = [
    36, // Kick: C2
    50, // Snare: D3
    60, // HiHat: C4
    33, // Bass: A1 (55 Hz)
];

/// Kick drum parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KickParams {
    pub pitch_start: f32,  // 80-250 Hz, default 150
    pub pitch_end: f32,    // 30-80 Hz, default 50
    pub pitch_decay: f32,  // 4-20, default 8 (how fast pitch drops)
    pub amp_decay: f32,    // 5-20, default 10 (overall decay time)
    pub click: f32,        // 0-1, default 0.3 (attack click amount)
    pub drive: f32,        // 0-1, default 0 (saturation)
}

impl Default for KickParams {
    fn default() -> Self {
        Self {
            pitch_start: 150.0,
            pitch_end: 50.0,
            pitch_decay: 8.0,
            amp_decay: 10.0,
            click: 0.3,
            drive: 0.0,
        }
    }
}

/// Snare drum parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnareParams {
    pub tone_freq: f32,    // 120-300 Hz, default 180
    pub tone_decay: f32,   // 10-40, default 20
    pub noise_decay: f32,  // 8-30, default 15
    pub tone_mix: f32,     // 0-1, default 0.4 (tone vs noise balance)
    pub snappy: f32,       // 0-1, default 0.6 (high freq emphasis)
}

impl Default for SnareParams {
    fn default() -> Self {
        Self {
            tone_freq: 180.0,
            tone_decay: 20.0,
            noise_decay: 15.0,
            tone_mix: 0.4,
            snappy: 0.6,
        }
    }
}

/// Hi-hat parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HiHatParams {
    pub decay: f32,  // 20-100, default 40 (envelope decay)
    pub tone: f32,   // 0-1, default 0.5 (filter brightness)
    pub open: f32,   // 0-1, default 0 (0=closed/short, 1=open/long)
}

impl Default for HiHatParams {
    fn default() -> Self {
        Self {
            decay: 40.0,
            tone: 0.5,
            open: 0.0,
        }
    }
}

/// Bass synth parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BassParams {
    pub frequency: f32, // 30-120 Hz, default 55 (A1)
    pub decay: f32,     // 3-12, default 6
    pub saw_mix: f32,   // 0-1, default 0.2 (sine vs saw)
    pub sub: f32,       // 0-1, default 0 (sub-octave)
}

impl Default for BassParams {
    fn default() -> Self {
        Self {
            frequency: 55.0,
            decay: 6.0,
            saw_mix: 0.2,
            sub: 0.0,
        }
    }
}

/// Parameter ID for addressing individual parameters
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamId {
    // Kick (track 0)
    KickPitchStart,
    KickPitchEnd,
    KickPitchDecay,
    KickAmpDecay,
    KickClick,
    KickDrive,
    // Snare (track 1)
    SnareToneFreq,
    SnareToneDecay,
    SnareNoiseDecay,
    SnareToneMix,
    SnareSnappy,
    // HiHat (track 2)
    HiHatDecay,
    HiHatTone,
    HiHatOpen,
    // Bass (track 3)
    BassFrequency,
    BassDecay,
    BassSawMix,
    BassSub,
}

impl ParamId {
    /// Human-readable parameter name
    pub fn name(&self) -> &'static str {
        match self {
            ParamId::KickPitchStart => "Pitch Start",
            ParamId::KickPitchEnd => "Pitch End",
            ParamId::KickPitchDecay => "Pitch Decay",
            ParamId::KickAmpDecay => "Amp Decay",
            ParamId::KickClick => "Click",
            ParamId::KickDrive => "Drive",
            ParamId::SnareToneFreq => "Tone Freq",
            ParamId::SnareToneDecay => "Tone Decay",
            ParamId::SnareNoiseDecay => "Noise Decay",
            ParamId::SnareToneMix => "Tone Mix",
            ParamId::SnareSnappy => "Snappy",
            ParamId::HiHatDecay => "Decay",
            ParamId::HiHatTone => "Tone",
            ParamId::HiHatOpen => "Open",
            ParamId::BassFrequency => "Frequency",
            ParamId::BassDecay => "Decay",
            ParamId::BassSawMix => "Saw Mix",
            ParamId::BassSub => "Sub",
        }
    }

    /// Short key name for MCP/serialization (with track prefix for uniqueness)
    pub fn key(&self) -> &'static str {
        match self {
            ParamId::KickPitchStart => "kick_pitch_start",
            ParamId::KickPitchEnd => "kick_pitch_end",
            ParamId::KickPitchDecay => "kick_pitch_decay",
            ParamId::KickAmpDecay => "kick_amp_decay",
            ParamId::KickClick => "kick_click",
            ParamId::KickDrive => "kick_drive",
            ParamId::SnareToneFreq => "snare_tone_freq",
            ParamId::SnareToneDecay => "snare_tone_decay",
            ParamId::SnareNoiseDecay => "snare_noise_decay",
            ParamId::SnareToneMix => "snare_tone_mix",
            ParamId::SnareSnappy => "snare_snappy",
            ParamId::HiHatDecay => "hihat_decay",
            ParamId::HiHatTone => "hihat_tone",
            ParamId::HiHatOpen => "hihat_open",
            ParamId::BassFrequency => "bass_frequency",
            ParamId::BassDecay => "bass_decay",
            ParamId::BassSawMix => "bass_saw_mix",
            ParamId::BassSub => "bass_sub",
        }
    }

    /// Parse parameter from full key string (e.g., "kick_pitch_start")
    pub fn from_key(key: &str) -> Option<ParamId> {
        match key {
            "kick_pitch_start" => Some(ParamId::KickPitchStart),
            "kick_pitch_end" => Some(ParamId::KickPitchEnd),
            "kick_pitch_decay" => Some(ParamId::KickPitchDecay),
            "kick_amp_decay" => Some(ParamId::KickAmpDecay),
            "kick_click" => Some(ParamId::KickClick),
            "kick_drive" => Some(ParamId::KickDrive),
            "snare_tone_freq" => Some(ParamId::SnareToneFreq),
            "snare_tone_decay" => Some(ParamId::SnareToneDecay),
            "snare_noise_decay" => Some(ParamId::SnareNoiseDecay),
            "snare_tone_mix" => Some(ParamId::SnareToneMix),
            "snare_snappy" => Some(ParamId::SnareSnappy),
            "hihat_decay" => Some(ParamId::HiHatDecay),
            "hihat_tone" => Some(ParamId::HiHatTone),
            "hihat_open" => Some(ParamId::HiHatOpen),
            "bass_frequency" => Some(ParamId::BassFrequency),
            "bass_decay" => Some(ParamId::BassDecay),
            "bass_saw_mix" => Some(ParamId::BassSawMix),
            "bass_sub" => Some(ParamId::BassSub),
            _ => None,
        }
    }

    /// Parameter range (min, max, default)
    pub fn range(&self) -> (f32, f32, f32) {
        match self {
            ParamId::KickPitchStart => (80.0, 250.0, 150.0),
            ParamId::KickPitchEnd => (30.0, 80.0, 50.0),
            ParamId::KickPitchDecay => (4.0, 20.0, 8.0),
            ParamId::KickAmpDecay => (5.0, 20.0, 10.0),
            ParamId::KickClick => (0.0, 1.0, 0.3),
            ParamId::KickDrive => (0.0, 1.0, 0.0),
            ParamId::SnareToneFreq => (120.0, 300.0, 180.0),
            ParamId::SnareToneDecay => (10.0, 40.0, 20.0),
            ParamId::SnareNoiseDecay => (8.0, 30.0, 15.0),
            ParamId::SnareToneMix => (0.0, 1.0, 0.4),
            ParamId::SnareSnappy => (0.0, 1.0, 0.6),
            ParamId::HiHatDecay => (20.0, 100.0, 40.0),
            ParamId::HiHatTone => (0.0, 1.0, 0.5),
            ParamId::HiHatOpen => (0.0, 1.0, 0.0),
            ParamId::BassFrequency => (30.0, 120.0, 55.0),
            ParamId::BassDecay => (3.0, 12.0, 6.0),
            ParamId::BassSawMix => (0.0, 1.0, 0.2),
            ParamId::BassSub => (0.0, 1.0, 0.0),
        }
    }

    /// Which track this parameter belongs to
    pub fn track(&self) -> usize {
        match self {
            ParamId::KickPitchStart
            | ParamId::KickPitchEnd
            | ParamId::KickPitchDecay
            | ParamId::KickAmpDecay
            | ParamId::KickClick
            | ParamId::KickDrive => 0,
            ParamId::SnareToneFreq
            | ParamId::SnareToneDecay
            | ParamId::SnareNoiseDecay
            | ParamId::SnareToneMix
            | ParamId::SnareSnappy => 1,
            ParamId::HiHatDecay | ParamId::HiHatTone | ParamId::HiHatOpen => 2,
            ParamId::BassFrequency | ParamId::BassDecay | ParamId::BassSawMix | ParamId::BassSub => {
                3
            }
        }
    }

    /// Get all parameters for a given track
    pub fn params_for_track(track: usize) -> Vec<ParamId> {
        match track {
            0 => vec![
                ParamId::KickPitchStart,
                ParamId::KickPitchEnd,
                ParamId::KickPitchDecay,
                ParamId::KickAmpDecay,
                ParamId::KickClick,
                ParamId::KickDrive,
            ],
            1 => vec![
                ParamId::SnareToneFreq,
                ParamId::SnareToneDecay,
                ParamId::SnareNoiseDecay,
                ParamId::SnareToneMix,
                ParamId::SnareSnappy,
            ],
            2 => vec![ParamId::HiHatDecay, ParamId::HiHatTone, ParamId::HiHatOpen],
            3 => vec![
                ParamId::BassFrequency,
                ParamId::BassDecay,
                ParamId::BassSawMix,
                ParamId::BassSub,
            ],
            _ => vec![],
        }
    }

    /// Parse parameter from track + key string
    pub fn from_track_key(track: usize, key: &str) -> Option<ParamId> {
        match (track, key) {
            (0, "pitch_start") => Some(ParamId::KickPitchStart),
            (0, "pitch_end") => Some(ParamId::KickPitchEnd),
            (0, "pitch_decay") => Some(ParamId::KickPitchDecay),
            (0, "amp_decay") => Some(ParamId::KickAmpDecay),
            (0, "click") => Some(ParamId::KickClick),
            (0, "drive") => Some(ParamId::KickDrive),
            (1, "tone_freq") => Some(ParamId::SnareToneFreq),
            (1, "tone_decay") => Some(ParamId::SnareToneDecay),
            (1, "noise_decay") => Some(ParamId::SnareNoiseDecay),
            (1, "tone_mix") => Some(ParamId::SnareToneMix),
            (1, "snappy") => Some(ParamId::SnareSnappy),
            (2, "decay") => Some(ParamId::HiHatDecay),
            (2, "tone") => Some(ParamId::HiHatTone),
            (2, "open") => Some(ParamId::HiHatOpen),
            (3, "frequency") => Some(ParamId::BassFrequency),
            (3, "decay") => Some(ParamId::BassDecay),
            (3, "saw_mix") => Some(ParamId::BassSawMix),
            (3, "sub") => Some(ParamId::BassSub),
            _ => None,
        }
    }
}
