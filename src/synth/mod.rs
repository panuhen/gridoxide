pub mod bass;
pub mod hihat;
pub mod kick;
pub mod params;
pub mod snare;

pub use bass::BassSynth;
pub use hihat::HiHatSynth;
pub use kick::KickSynth;
pub use params::{BassParams, HiHatParams, KickParams, ParamId, SnareParams};
pub use snare::SnareSynth;
