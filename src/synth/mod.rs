pub mod bass;
pub mod hihat;
pub mod kick;
pub mod params;
pub mod sampler;
pub mod snare;
pub mod source;

pub use params::{note_name, BassParams, HiHatParams, KickParams, SnareParams};
pub use sampler::load_wav;
pub use source::{create_synth, ParamDescriptor, SoundSource, SynthType};
