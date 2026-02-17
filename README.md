# Gridoxide

A Rust-based terminal step sequencer and synthesizer for EDM production.

## Overview

Gridoxide is a terminal EDM production studio designed for collaborative use between humans (via TUI) and Claude (via MCP tools). The core principle is **full parity between TUI and MCP** - every state-mutating action flows through the same command bus regardless of source.

When the TUI is running, an MCP socket bridge at `/tmp/gridoxide.sock` allows Claude to control the same session in real time - changes from MCP appear live in the grid.

## Current Status: v0.8.3 (Phase 8d)

- **Per-step velocity**: Each step has velocity (0-127) affecting volume/intensity
- **Per-step probability**: Each step has trigger probability (0-100%)
- **A/B pattern variations**: Each pattern has two variations that can be toggled/copied
- **Dynamic tracks**: Add/remove tracks at runtime (kick, snare, hihat, bass, sampler)
- **Sampler synth**: WAV sample loading with pitch shifting, ADSR envelope, loop mode
- **ADSR envelope**: Attack, Decay, Sustain level, Release for samplers
- **Loop mode**: Configurable loop start/end points, hold_steps for sustained playback
- **16-slot pattern bank**: Copy, clear, switch patterns
- **Song mode**: Arrangement with pattern chaining and repeat counts
- **Project I/O**: Save/load .grox JSON files, export WAV audio
- **Sample browser**: TUI overlay for browsing and loading WAV files
- 16-step pattern grid with per-step MIDI notes (0-127)
- Per-track FX chains: filter (LP/HP/BP), distortion (tanh), delay (ring buffer)
- Master bus reverb (Schroeder)
- Signal chain: Synth → [Filter → Distortion → Delay] → Volume → Pan → Sum → [Reverb] → Soft Clip
- Mixer with volume, pan, mute/solo
- Command bus architecture with event logging
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
| v / V | Velocity down / up (±16) |
| b / B | Probability down / up (±10%) |
| x | Toggle A/B variation |
| X (Shift+x) | Copy current variation to other |
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
| 1-9,0 | Select track |
| Up/Down / jk | Select parameter |
| Left/Right / hl | Adjust value (fine ±5%) |
| [ / ] | Adjust value (coarse ±20%) |
| F | Toggle filter on/off |
| D | Toggle distortion on/off |
| Y | Toggle delay on/off |
| R | Toggle master reverb on/off |
| P | Play/Stop toggle |
| S | Stop |
| Tab | Switch to Song view |
| Esc | Back to Grid view |
| Q | Quit |

### Song View
| Key | Action |
|-----|--------|
| Up/Down / jk | Navigate arrangement entries |
| Left/Right / hl | Adjust pattern/repeats |
| Enter | Append new entry |
| Delete/Backspace | Remove entry |
| 0-9 | Quick select pattern slot |
| M | Toggle pattern/song mode |
| P | Play/Stop toggle |
| S | Stop |
| Tab | Switch to Help view |
| Esc | Back to Grid view |

### Project Controls (All Views)
| Key | Action |
|-----|--------|
| Ctrl+S | Save project |
| Ctrl+O | Open project |
| Ctrl+E | Export WAV (pattern) |
| Ctrl+W | Export WAV (song) |
| Shift+L | Open sample browser (sampler tracks) |

### Sampler Parameters
When using a sampler track, these parameters control playback:
- **Amplitude**: Output volume (0.0-1.0)
- **Attack**: Fade-in time (0-50ms)
- **Decay**: Time to reach sustain level (10-500ms)
- **Sustain**: Held volume level (0.0-1.0)
- **Release**: Fade-out time after note-off (10-2000ms)
- **Start/End Point**: Sample region (0.0-1.0)
- **Pitch Shift**: Transpose in semitones (-24 to +24)
- **Loop**: Enable looping playback
- **Loop Start/End**: Loop region within sample
- **Hold Steps**: Steps before auto-release (1-16)

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

**Per-Step Data:**
- `set_step_note` - Set MIDI note (0-127) for a step
- `set_step_velocity` - Set velocity (0-127) for a step
- `set_step_probability` - Set trigger probability (0-100%) for a step
- `get_step_notes` - Get all step data for a track (notes, velocity, probability)

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

**Pattern Bank:**
- `select_pattern` - Switch active pattern (0-15)
- `get_pattern_bank` - Overview of all 16 pattern slots
- `copy_pattern` - Copy pattern from src to dst slot
- `clear_pattern` - Clear all tracks in a pattern

**Arrangement:**
- `get_arrangement` - Get full song arrangement
- `append_arrangement` - Add pattern entry to end
- `insert_arrangement` - Insert entry at position
- `remove_arrangement` - Remove entry
- `set_arrangement_entry` - Modify existing entry
- `clear_arrangement` - Clear all entries
- `set_playback_mode` - Switch between "pattern" and "song" mode

**Variations:**
- `set_variation` - Select variation A or B
- `toggle_variation` - Switch between A and B
- `copy_variation` - Copy one variation to another

**Dynamic Tracks:**
- `add_track` - Add new track (kick, snare, hihat, bass, sampler)
- `remove_track` - Remove track by index

**Sampler:**
- `load_sample` - Load WAV file into sampler track
- `preview_sample` - Audition sample without loading
- `list_samples` - List available samples in search directories

**Project I/O:**
- `save_project` - Save to .grox JSON file
- `load_project` - Load from .grox file
- `export_wav` - Render and export audio (pattern or song mode)
- `list_projects` - List .grox files in directory

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
| 4.5 | Per-step notes | Complete |
| 5 | Effects | Complete |
| 6 | Patterns + Arrangement | Complete |
| 7 | Project I/O | Complete |
| 8a | Dynamic Tracks | Complete |
| 8b | Sampler Synth | Complete |
| 8c | Sampler ADSR + Loop | Complete |
| 8d | Step Data & Variations | **Complete** |
| 8e | Timeline | Planned |
| 9 | Polish | Planned |

## License

MIT
