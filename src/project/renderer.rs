use std::path::Path;

use anyhow::{Context, Result};

use crate::audio::SequencerState;
use crate::fx::{configure_fx_chain, StereoReverb, TrackFxChain};
use crate::samples;
use crate::sequencer::{Clock, STEPS};
use crate::synth::{create_synth, load_wav, SoundSource, SynthType};

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
    synths: Vec<Box<dyn SoundSource>>,
    clock: Clock,
    fx_chains: Vec<TrackFxChain>,
    reverb: StereoReverb,
    reverb_enabled: bool,
    volumes: Vec<f32>,
    pans: Vec<f32>,
    mutes: Vec<bool>,
    solos: Vec<bool>,
}

impl OfflineRenderer {
    fn from_state(state: &SequencerState) -> Self {
        let mut synths: Vec<Box<dyn SoundSource>> = Vec::with_capacity(state.tracks.len());
        let mut volumes = Vec::with_capacity(state.tracks.len());
        let mut pans = Vec::with_capacity(state.tracks.len());
        let mut mutes = Vec::with_capacity(state.tracks.len());
        let mut solos = Vec::with_capacity(state.tracks.len());
        let mut fx_chains = Vec::with_capacity(state.tracks.len());

        for track in &state.tracks {
            let mut synth = create_synth(track.synth_type, SAMPLE_RATE, Some(&track.params_snapshot));
            // Load sample buffer for sampler tracks
            if track.synth_type == SynthType::Sampler {
                if let Some(wav_path) = track.params_snapshot.get("wav_path").and_then(|v| v.as_str()) {
                    if !wav_path.is_empty() {
                        // Try absolute, then sample dirs
                        let path = std::path::PathBuf::from(wav_path);
                        let resolved = if path.exists() {
                            Some(path)
                        } else {
                            let dirs = samples::search_dirs();
                            samples::resolve_sample_path(wav_path, &dirs)
                        };
                        if let Some(full_path) = resolved {
                            if let Ok(buffer) = load_wav(&full_path, SAMPLE_RATE) {
                                let path_str = full_path.to_string_lossy().to_string();
                                synth.load_buffer(buffer, &path_str);
                            }
                        }
                    }
                }
            }
            synths.push(synth);
            volumes.push(track.volume);
            pans.push(track.pan);
            mutes.push(track.mute);
            solos.push(track.solo);
            let mut chain = TrackFxChain::new(SAMPLE_RATE);
            configure_fx_chain(&mut chain, &track.fx);
            fx_chains.push(chain);
        }

        let clock = Clock::new(SAMPLE_RATE, state.bpm);

        let mut reverb = StereoReverb::new(SAMPLE_RATE);
        reverb.set_decay(state.master_fx.reverb_decay);
        reverb.set_mix(state.master_fx.reverb_mix);
        reverb.set_damping(state.master_fx.reverb_damping);

        Self {
            synths,
            clock,
            fx_chains,
            reverb,
            reverb_enabled: state.master_fx.reverb_enabled,
            volumes,
            pans,
            mutes,
            solos,
        }
    }

    /// Render a fixed number of samples, using the given pattern for triggering
    fn render(
        &mut self,
        state: &SequencerState,
        mode: &ExportMode,
    ) -> Vec<(f32, f32)> {
        let tail_samples = (SAMPLE_RATE * TAIL_SECONDS) as usize;
        let num_tracks = self.synths.len();

        // Calculate total pattern steps to render
        let total_steps = match mode {
            ExportMode::Pattern(_idx) => {
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
                    for i in 0..num_tracks {
                        let sd = pat.get_step(i, step);
                        if sd.active {
                            self.synths[i].trigger_with_note(sd.note);
                        }
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
                            }
                        }
                    }
                }
            } else {
                // In tail: just advance clock without triggering
                self.clock.tick();
                self.clock.take_pattern_wrap();
            }

            // Generate audio (always, including tail for decay)
            let any_solo = self.solos.iter().any(|&s| s);
            let mut left = 0.0f32;
            let mut right = 0.0f32;
            for i in 0..num_tracks {
                let raw = self.fx_chains[i].process(self.synths[i].next_sample());
                let audible = if any_solo {
                    self.solos[i]
                } else {
                    !self.mutes[i]
                };
                if !audible {
                    continue;
                }
                let s = raw * self.volumes[i];
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
