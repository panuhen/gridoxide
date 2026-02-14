pub mod bass;
pub mod hihat;
pub mod kick;
pub mod params;
pub mod snare;

pub use bass::BassSynth;
pub use hihat::HiHatSynth;
pub use kick::KickSynth;
pub use params::{midi_to_freq, note_name, BassParams, HiHatParams, KickParams, ParamId, SnareParams, DEFAULT_NOTES};
pub use snare::SnareSynth;
