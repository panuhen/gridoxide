pub mod bass;
pub mod hihat;
pub mod kick;
pub mod params;
pub mod snare;
pub mod source;

pub use params::{note_name, BassParams, HiHatParams, KickParams, SnareParams};
pub use source::{create_synth, ParamDescriptor, SoundSource, SynthType};
