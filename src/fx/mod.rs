pub mod delay;
pub mod distortion;
pub mod filter;
pub mod reverb;

pub use delay::Delay;
pub use distortion::Distortion;
pub use filter::{FilterType, SvfFilter};
pub use reverb::StereoReverb;

use serde::{Deserialize, Serialize};

/// Which effect to toggle on a track
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxType {
    Filter,
    Distortion,
    Delay,
}

impl FxType {
    pub fn name(&self) -> &'static str {
        match self {
            FxType::Filter => "filter",
            FxType::Distortion => "distortion",
            FxType::Delay => "delay",
        }
    }
}

/// FX parameter identifiers for per-track effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxParamId {
    FilterCutoff,
    FilterResonance,
    DistDrive,
    DistMix,
    DelayTime,
    DelayFeedback,
    DelayMix,
}

impl FxParamId {
    pub fn name(&self) -> &'static str {
        match self {
            FxParamId::FilterCutoff => "Cutoff",
            FxParamId::FilterResonance => "Resonance",
            FxParamId::DistDrive => "Drive",
            FxParamId::DistMix => "Dist Mix",
            FxParamId::DelayTime => "Time",
            FxParamId::DelayFeedback => "Feedback",
            FxParamId::DelayMix => "Delay Mix",
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            FxParamId::FilterCutoff => "filter_cutoff",
            FxParamId::FilterResonance => "filter_resonance",
            FxParamId::DistDrive => "dist_drive",
            FxParamId::DistMix => "dist_mix",
            FxParamId::DelayTime => "delay_time",
            FxParamId::DelayFeedback => "delay_feedback",
            FxParamId::DelayMix => "delay_mix",
        }
    }

    /// Returns (min, max, default) for this parameter
    pub fn range(&self) -> (f32, f32, f32) {
        match self {
            FxParamId::FilterCutoff => (20.0, 20000.0, 2000.0),
            FxParamId::FilterResonance => (0.0, 0.95, 0.2),
            FxParamId::DistDrive => (0.0, 1.0, 0.1),
            FxParamId::DistMix => (0.0, 1.0, 0.5),
            FxParamId::DelayTime => (10.0, 500.0, 200.0),
            FxParamId::DelayFeedback => (0.0, 0.9, 0.3),
            FxParamId::DelayMix => (0.0, 1.0, 0.2),
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "filter_cutoff" => Some(FxParamId::FilterCutoff),
            "filter_resonance" => Some(FxParamId::FilterResonance),
            "dist_drive" => Some(FxParamId::DistDrive),
            "dist_mix" => Some(FxParamId::DistMix),
            "delay_time" => Some(FxParamId::DelayTime),
            "delay_feedback" => Some(FxParamId::DelayFeedback),
            "delay_mix" => Some(FxParamId::DelayMix),
            _ => None,
        }
    }

    /// All FX params in display order
    pub fn all() -> Vec<FxParamId> {
        vec![
            FxParamId::FilterCutoff,
            FxParamId::FilterResonance,
            FxParamId::DistDrive,
            FxParamId::DistMix,
            FxParamId::DelayTime,
            FxParamId::DelayFeedback,
            FxParamId::DelayMix,
        ]
    }
}

/// Master FX parameter identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MasterFxParamId {
    ReverbDecay,
    ReverbMix,
    ReverbDamping,
}

impl MasterFxParamId {
    pub fn name(&self) -> &'static str {
        match self {
            MasterFxParamId::ReverbDecay => "Decay",
            MasterFxParamId::ReverbMix => "Mix",
            MasterFxParamId::ReverbDamping => "Damping",
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            MasterFxParamId::ReverbDecay => "reverb_decay",
            MasterFxParamId::ReverbMix => "reverb_mix",
            MasterFxParamId::ReverbDamping => "reverb_damping",
        }
    }

    pub fn range(&self) -> (f32, f32, f32) {
        match self {
            MasterFxParamId::ReverbDecay => (0.1, 0.95, 0.5),
            MasterFxParamId::ReverbMix => (0.0, 1.0, 0.3),
            MasterFxParamId::ReverbDamping => (0.0, 1.0, 0.5),
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "reverb_decay" => Some(MasterFxParamId::ReverbDecay),
            "reverb_mix" => Some(MasterFxParamId::ReverbMix),
            "reverb_damping" => Some(MasterFxParamId::ReverbDamping),
            _ => None,
        }
    }

    pub fn all() -> Vec<MasterFxParamId> {
        vec![
            MasterFxParamId::ReverbDecay,
            MasterFxParamId::ReverbMix,
            MasterFxParamId::ReverbDamping,
        ]
    }
}

/// Per-track FX state (shared between audio thread and UI/MCP)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackFxState {
    pub filter_enabled: bool,
    pub filter_type: FilterType,
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub dist_enabled: bool,
    pub dist_drive: f32,
    pub dist_mix: f32,
    pub delay_enabled: bool,
    pub delay_time: f32,
    pub delay_feedback: f32,
    pub delay_mix: f32,
}

impl Default for TrackFxState {
    fn default() -> Self {
        Self {
            filter_enabled: false,
            filter_type: FilterType::LowPass,
            filter_cutoff: 2000.0,
            filter_resonance: 0.2,
            dist_enabled: false,
            dist_drive: 0.1,
            dist_mix: 0.5,
            delay_enabled: false,
            delay_time: 200.0,
            delay_feedback: 0.3,
            delay_mix: 0.2,
        }
    }
}

/// Master FX state (shared between audio thread and UI/MCP)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasterFxState {
    pub reverb_enabled: bool,
    pub reverb_decay: f32,
    pub reverb_mix: f32,
    pub reverb_damping: f32,
}

impl Default for MasterFxState {
    fn default() -> Self {
        Self {
            reverb_enabled: false,
            reverb_decay: 0.5,
            reverb_mix: 0.3,
            reverb_damping: 0.5,
        }
    }
}

/// Per-track FX processing chain (owns DSP instances)
pub struct TrackFxChain {
    pub filter: SvfFilter,
    pub distortion: Distortion,
    pub delay: Delay,
    pub filter_enabled: bool,
    pub dist_enabled: bool,
    pub delay_enabled: bool,
}

impl TrackFxChain {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            filter: SvfFilter::new(sample_rate),
            distortion: Distortion::new(),
            delay: Delay::new(sample_rate),
            filter_enabled: false,
            dist_enabled: false,
            delay_enabled: false,
        }
    }

    /// Process a mono sample through the FX chain: Filter -> Distortion -> Delay
    pub fn process(&mut self, input: f32) -> f32 {
        let mut s = input;
        if self.filter_enabled {
            s = self.filter.process(s);
        }
        if self.dist_enabled {
            s = self.distortion.process(s);
        }
        if self.delay_enabled {
            s = self.delay.process(s);
        }
        s
    }
}

/// Configure a TrackFxChain from a TrackFxState snapshot.
/// Used by both the LoadProject handler and the offline renderer.
pub fn configure_fx_chain(chain: &mut TrackFxChain, state: &TrackFxState) {
    chain.filter_enabled = state.filter_enabled;
    chain.filter.set_filter_type(state.filter_type);
    chain.filter.set_cutoff(state.filter_cutoff);
    chain.filter.set_resonance(state.filter_resonance);
    chain.dist_enabled = state.dist_enabled;
    chain.distortion.set_drive(state.dist_drive);
    chain.distortion.set_mix(state.dist_mix);
    chain.delay_enabled = state.delay_enabled;
    chain.delay.set_time(state.delay_time);
    chain.delay.set_feedback(state.delay_feedback);
    chain.delay.set_mix(state.delay_mix);
}
