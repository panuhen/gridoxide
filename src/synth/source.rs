use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::bass::BassSynth;
use super::hihat::HiHatSynth;
use super::kick::KickSynth;
use super::sampler::SamplerSynth;
use super::snare::SnareSynth;

/// Identifies the type of synthesizer
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SynthType {
    Kick,
    Snare,
    HiHat,
    Bass,
    Sampler,
}

impl SynthType {
    pub fn name(&self) -> &'static str {
        match self {
            SynthType::Kick => "kick",
            SynthType::Snare => "snare",
            SynthType::HiHat => "hihat",
            SynthType::Bass => "bass",
            SynthType::Sampler => "sampler",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SynthType::Kick => "KICK",
            SynthType::Snare => "SNARE",
            SynthType::HiHat => "HIHAT",
            SynthType::Bass => "BASS",
            SynthType::Sampler => "SAMPLER",
        }
    }

    pub fn from_name(name: &str) -> Option<SynthType> {
        match name {
            "kick" => Some(SynthType::Kick),
            "snare" => Some(SynthType::Snare),
            "hihat" => Some(SynthType::HiHat),
            "bass" => Some(SynthType::Bass),
            "sampler" => Some(SynthType::Sampler),
            _ => None,
        }
    }
}

/// Describes a synth parameter with its range and metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParamDescriptor {
    pub key: String,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Trait for all sound sources in gridoxide.
/// Must be Send for use on the audio thread.
pub trait SoundSource: Send {
    /// The type of synth
    fn synth_type(&self) -> SynthType;

    /// Display name for this synth instance
    fn type_name(&self) -> &'static str;

    /// Default MIDI note for this synth
    fn default_note(&self) -> u8;

    /// Trigger the synth (uses default note behavior)
    fn trigger(&mut self);

    /// Trigger with a specific MIDI note
    fn trigger_with_note(&mut self, note: u8);

    /// Generate the next audio sample
    fn next_sample(&mut self) -> f32;

    /// Get descriptors for all parameters
    fn param_descriptors(&self) -> Vec<ParamDescriptor>;

    /// Get a parameter value by key
    fn get_param(&self, key: &str) -> Option<f32>;

    /// Set a parameter value by key. Returns true if the key was recognized.
    fn set_param(&mut self, key: &str, value: f32) -> bool;

    /// Serialize all parameters to JSON
    fn serialize_params(&self) -> Value;

    /// Deserialize parameters from JSON
    fn deserialize_params(&mut self, params: &Value);

    /// Load a sample buffer into this synth (only used by SamplerSynth, no-op for others)
    fn load_buffer(&mut self, _buffer: Vec<f32>, _path: &str) {}

    /// Called on each sequencer step tick. Used by samplers for hold_steps countdown.
    fn step_tick(&mut self) {}
}

/// Factory function: create a synth from its type, sample rate, and optional saved params
pub fn create_synth(
    synth_type: SynthType,
    sample_rate: f32,
    params_json: Option<&Value>,
) -> Box<dyn SoundSource> {
    let mut synth: Box<dyn SoundSource> = match synth_type {
        SynthType::Kick => Box::new(KickSynth::new(sample_rate)),
        SynthType::Snare => Box::new(SnareSynth::new(sample_rate)),
        SynthType::HiHat => Box::new(HiHatSynth::new(sample_rate)),
        SynthType::Bass => Box::new(BassSynth::new(sample_rate)),
        SynthType::Sampler => Box::new(SamplerSynth::new(sample_rate)),
    };
    if let Some(params) = params_json {
        synth.deserialize_params(params);
    }
    synth
}
