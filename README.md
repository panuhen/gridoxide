# Gridoxide

A Rust-based terminal step sequencer and synthesizer for EDM production.

## Overview

Gridoxide is a terminal EDM production studio designed for collaborative use between humans (via TUI) and Claude (via MCP tools). The core principle is **full parity between TUI and MCP** - every state-mutating action flows through the same command bus regardless of source.

When the TUI is running, an MCP socket bridge at `/tmp/gridoxide.sock` allows Claude to control the same session in real time - changes from MCP appear live in the grid.

## Current Status: Phase 5

- 4 tracks: Kick, Snare, Hi-hat, Bass
- 16-step pattern grid with per-step MIDI notes (0-127)
- Melodic bass lines, tuned kicks, pitched snares, brightness-varied hihats
- Grid view with note names on active steps
- Parameter editor (Params view)
- Mixer with volume, pan, mute/solo (Mixer view)
- Per-track FX chains: filter (LP/HP/BP), distortion (tanh), delay (ring buffer)
- Master bus reverb (Schroeder)
- FX view for real-time effect tweaking
- Signal chain: Synth → [Filter → Distortion → Delay] → Volume → Pan → Sum → [Reverb] → Soft Clip
- Real-time synth parameter tweaking
- BPM clock with play/stop
- Command bus architecture
- Event logging for MCP "listening"
- MCP server with full tool suite
- Unified TUI+MCP socket bridge (shared state)

## Installation

```bash
cd /path/to/gridoxide
cargo install --path .

# Now run from anywhere:
gridoxide
```

## Usage

```bash
# Run with default theme
gridoxide

# Run with a specific theme
gridoxide --theme phosphor-green

# List available themes
gridoxide --list-themes

# Run as MCP server (connects to TUI if running, otherwise standalone)
gridoxide --mcp
```

## Controls

### Grid View
| Key | Action |
|-----|--------|
| Arrow keys / hjkl | Navigate grid |
| Space / Enter | Toggle step at cursor |
| ] | Note up 1 semitone |
| [ | Note down 1 semitone |
| } (Shift+]) | Note up 1 octave |
| { (Shift+[) | Note down 1 octave |
| P | Play/Stop toggle |
| S | Stop (reset to step 0) |
| +/- | Adjust BPM |
| C | Clear current track |
| F | Fill current track |
| Tab / E | Switch to Params view |
| Q / Esc | Quit |

### Params View
| Key | Action |
|-----|--------|
| 1-4 | Select track |
| Up/Down / jk | Select parameter |
| Left/Right / hl | Adjust value (fine ±5%) |
| [ / ] | Adjust value (coarse ±20%) |
| P | Play/Stop toggle |
| S | Stop |
| Tab | Switch to Mixer view |
| Esc | Back to Grid view |
| Q | Quit |

### Mixer View
| Key | Action |
|-----|--------|
| 1-4 | Select track |
| Up/Down / jk | Select field |
| Left/Right / hl | Adjust value |
| M | Toggle mute |
| O | Toggle solo |
| P | Play/Stop toggle |
| S | Stop |
| Tab | Switch to FX view |
| Esc | Back to Grid view |
| Q | Quit |

### FX View
| Key | Action |
|-----|--------|
| 1-4 | Select track |
| Up/Down / jk | Select parameter |
| Left/Right / hl | Adjust value (fine ±5%) |
| [ / ] | Adjust value (coarse ±20%) |
| F | Toggle filter on/off |
| D | Toggle distortion on/off |
| Y | Toggle delay on/off |
| R | Toggle master reverb on/off |
| P | Play/Stop toggle |
| S | Stop |
| Tab | Back to Grid view |
| Esc | Back to Grid view |
| Q | Quit |

## MCP Tools

When running with `--mcp`, gridoxide exposes these tools. If the TUI is running, MCP commands go through a socket bridge to share the same session.

**Transport:**
- `play` - Start playback
- `stop` - Stop and reset
- `set_bpm` - Set tempo (60-200)
- `get_state` - Get current state

**Pattern:**
- `toggle_step` - Toggle step on/off (optional `note` parameter)
- `get_pattern` - Get full grid with note data
- `clear_track` - Clear a track
- `fill_track` - Fill a track

**Per-Step Notes:**
- `set_step_note` - Set MIDI note (0-127) for a step
- `get_step_notes` - Get all step data for a track including notes

**Track Parameters:**
- `list_tracks` - List all tracks with available parameters
- `get_track_params` - Get params for a track with values and ranges
- `set_param` - Set a parameter (e.g., `kick_pitch_start`, `snare_snappy`)
- `reset_track` - Reset track to default parameters

**Mixer:**
- `get_mixer` - Get all mixer state
- `set_volume` - Set track volume (0.0-1.0)
- `set_pan` - Set track pan (-1.0 to 1.0)
- `toggle_mute` - Toggle track mute
- `toggle_solo` - Toggle track solo

**Per-Track FX:**
- `get_fx_params` - Get all FX parameters for a track (filter, distortion, delay)
- `set_fx_param` - Set an FX parameter (e.g., `filter_cutoff`, `dist_drive`, `delay_time`)
- `toggle_fx` - Toggle an effect on/off (`filter`, `distortion`, or `delay`)

**Master FX:**
- `get_master_fx_params` - Get master bus FX parameters (reverb)
- `set_master_fx_param` - Set a master FX parameter (`reverb_decay`, `reverb_mix`, `reverb_damping`)
- `toggle_master_fx` - Toggle master reverb on/off

**Events:**
- `get_events` - Get recent events (for "listening" to human actions)

## Themes

- `default` - Uses terminal's ANSI colors
- `phosphor-green` - Classic green CRT
- `amber-crt` - Warm amber monochrome
- `blue-terminal` - Cool blue tones
- `high-contrast` - Stark black and white

## Architecture

```
TUI (ratatui) ──┐
                ├──▶ Command Bus ──▶ Audio Engine (cpal)
MCP (tools)  ───┘         │
                          ▼
                     Event Log ◀── Claude "listens"
```

When the TUI is running, it opens a Unix socket at `/tmp/gridoxide.sock`. The `--mcp` process connects to this socket, so both TUI and MCP share the same command bus and audio engine. If the TUI is not running, `--mcp` falls back to a standalone audio engine.

## Roadmap

| Phase | Name | Status |
|-------|------|--------|
| 1 | Sound comes out | Complete |
| 2 | Grid sequencer | Complete |
| 3 | Sound shaping | Complete |
| 4 | Mixing | Complete |
| 4.5 | Per-step notes | **Complete** |
| 5 | Effects | **Complete** |
| 6 | Patterns + Arrangement | Planned |
| 7 | Project I/O | Planned |
| 8 | MIDI | Planned |
| 9 | Polish | Planned |

## License

MIT
