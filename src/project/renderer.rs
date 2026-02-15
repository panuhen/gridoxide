use std::path::Path;

use anyhow::{Context, Result};

use crate::audio::SequencerState;
use crate::fx::{configure_fx_chain, StereoReverb, TrackFxChain};
use crate::sequencer::{Clock, STEPS};
use crate::synth::{BassSynth, HiHatSynth, KickSynth, SnareSynth};

const SAMPLE_RATE: f32 = 44100.0;
const TAIL_SECONDS: f32 = 1.0;

/// What to render
pub enum ExportMode {
    /// Single pattern loop (by index) + decay tail
    Pattern(usize),
    /// Full arrangement + decay tail
    Song,
}

/// Result of an export operation
pub struct ExportResult {
    pub duration_secs: f32,
    pub samples: usize,
}

/// Offline renderer that mirrors the real-time audio callback
struct OfflineRenderer {
    kick: KickSynth,
    snare: SnareSynth,
    hihat: HiHatSynth,
    bass: BassSynth,
    clock: Clock,
    fx_chains: [TrackFxChain; 4],
    reverb: StereoReverb,
    reverb_enabled: bool,
    volumes: [f32; 4],
    pans: [f32; 4],
    mutes: [bool; 4],
    solos: [bool; 4],
}

impl OfflineRenderer {
    fn from_state(state: &SequencerState) -> Self {
        let mut kick = KickSynth::new(SAMPLE_RATE);
        kick.set_params(state.kick_params.clone());
        let mut snare = SnareSynth::new(SAMPLE_RATE);
        snare.set_params(state.snare_params.clone());
        let mut hihat = HiHatSynth::new(SAMPLE_RATE);
        hihat.set_params(state.hihat_params.clone());
        let mut bass = BassSynth::new(SAMPLE_RATE);
        bass.set_params(state.bass_params.clone());

        let clock = Clock::new(SAMPLE_RATE, state.bpm);

        let mut fx_chains = [
            TrackFxChain::new(SAMPLE_RATE),
            TrackFxChain::new(SAMPLE_RATE),
            TrackFxChain::new(SAMPLE_RATE),
            TrackFxChain::new(SAMPLE_RATE),
        ];
        for i in 0..4 {
            configure_fx_chain(&mut fx_chains[i], &state.track_fx[i]);
        }

        let mut reverb = StereoReverb::new(SAMPLE_RATE);
        reverb.set_decay(state.master_fx.reverb_decay);
        reverb.set_mix(state.master_fx.reverb_mix);
        reverb.set_damping(state.master_fx.reverb_damping);

        Self {
            kick,
            snare,
            hihat,
            bass,
            clock,
            fx_chains,
            reverb,
            reverb_enabled: state.master_fx.reverb_enabled,
            volumes: state.track_volumes,
            pans: state.track_pans,
            mutes: state.track_mutes,
            solos: state.track_solos,
        }
    }

    /// Render a fixed number of samples, using the given pattern for triggering
    fn render(
        &mut self,
        state: &SequencerState,
        mode: &ExportMode,
    ) -> Vec<(f32, f32)> {
        let tail_samples = (SAMPLE_RATE * TAIL_SECONDS) as usize;

        // Calculate total pattern steps to render
        let total_steps = match mode {
            ExportMode::Pattern(idx) => {
                let _ = idx; // pattern data accessed via state.pattern_bank
                STEPS // one loop = 16 steps
            }
            ExportMode::Song => {
                if state.arrangement.is_empty() {
                    STEPS // fallback: one pattern
                } else {
                    state
                        .arrangement
                        .entries
                        .iter()
                        .map(|e| e.repeats * STEPS)
                        .sum()
                }
            }
        };

        // samples per step
        let samples_per_beat = SAMPLE_RATE * 60.0 / state.bpm;
        let samples_per_step = samples_per_beat / 4.0;
        let content_samples = (total_steps as f32 * samples_per_step) as usize;
        let total_samples = content_samples + tail_samples;

        let mut output = Vec::with_capacity(total_samples);

        // Pattern tracking for song mode
        let mut current_pattern_idx = match mode {
            ExportMode::Pattern(idx) => *idx,
            ExportMode::Song => {
                if state.arrangement.is_empty() {
                    state.current_pattern
                } else {
                    state.arrangement.entries[0].pattern
                }
            }
        };
        let mut arrangement_pos: usize = 0;
        let mut arrangement_repeat: usize = 0;

        self.clock.play();

        for sample_idx in 0..total_samples {
            let in_content = sample_idx < content_samples;

            if in_content {
                // Check for step trigger
                if let Some(step) = self.clock.tick() {
                    let pat = state.pattern_bank.get(current_pattern_idx);
                    let s0 = pat.get_step(0, step);
                    if s0.active {
                        self.kick.trigger_with_note(s0.note);
                    }
                    let s1 = pat.get_step(1, step);
                    if s1.active {
                        self.snare.trigger_with_note(s1.note);
                    }
                    let s2 = pat.get_step(2, step);
                    if s2.active {
                        self.hihat.trigger_with_note(s2.note);
                    }
                    let s3 = pat.get_step(3, step);
                    if s3.active {
                        self.bass.trigger_with_note(s3.note);
                    }
                }

                // Pattern boundary logic for song mode
                if self.clock.take_pattern_wrap() {
                    if let ExportMode::Song = mode {
                        if !state.arrangement.is_empty() {
                            let entry = state.arrangement.entries[arrangement_pos];
                            arrangement_repeat += 1;
                            if arrangement_repeat >= entry.repeats {
                                arrangement_repeat = 0;
                                arrangement_pos += 1;
                                if arrangement_pos < state.arrangement.len() {
                                    current_pattern_idx =
                                        state.arrangement.entries[arrangement_pos].pattern;
                                }
                                // If we've passed the end, the content_samples limit
                                // will stop triggering new steps
                            }
                        }
                    }
                }
            } else {
                // In tail: just advance clock without triggering (for take_pattern_wrap)
                self.clock.tick();
                self.clock.take_pattern_wrap();
            }

            // Generate audio (always, including tail for decay)
            let raw = [
                self.fx_chains[0].process(self.kick.next_sample()),
                self.fx_chains[1].process(self.snare.next_sample()),
                self.fx_chains[2].process(self.hihat.next_sample()),
                self.fx_chains[3].process(self.bass.next_sample()),
            ];

            let any_solo = self.solos.iter().any(|&s| s);
            let mut left = 0.0f32;
            let mut right = 0.0f32;
            for i in 0..4 {
                let audible = if any_solo {
                    self.solos[i]
                } else {
                    !self.mutes[i]
                };
                if !audible {
                    continue;
                }
                let s = raw[i] * self.volumes[i];
                let angle = (self.pans[i] + 1.0) * 0.25 * std::f32::consts::PI;
                left += s * angle.cos();
                right += s * angle.sin();
            }

            if self.reverb_enabled {
                let (rl, rr) = self.reverb.process_stereo(left, right);
                left = rl;
                right = rr;
            }

            left = soft_clip(left);
            right = soft_clip(right);

            output.push((left, right));
        }

        output
    }
}

fn soft_clip(x: f32) -> f32 {
    if x > 1.0 {
        1.0 - (-x + 1.0).exp() * 0.5
    } else if x < -1.0 {
        -1.0 + (x + 1.0).exp() * 0.5
    } else {
        x
    }
}

/// Render and export audio as a WAV file
pub fn export_wav(
    state: &SequencerState,
    mode: ExportMode,
    path: &Path,
) -> Result<ExportResult> {
    let mut renderer = OfflineRenderer::from_state(state);
    let samples = renderer.render(state, &mode);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("Failed to create WAV file: {}", path.display()))?;

    for (left, right) in &samples {
        let l = (*left * 32767.0).clamp(-32768.0, 32767.0) as i16;
        let r = (*right * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(l)?;
        writer.write_sample(r)?;
    }

    writer.finalize()
        .with_context(|| format!("Failed to finalize WAV file: {}", path.display()))?;

    let duration_secs = samples.len() as f32 / SAMPLE_RATE;

    Ok(ExportResult {
        duration_secs,
        samples: samples.len(),
    })
}
