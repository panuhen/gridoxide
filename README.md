# Gridoxide

A Rust-based terminal step sequencer and synthesizer for EDM production.

## Overview

Gridoxide is a terminal EDM production studio designed for collaborative use between humans (via TUI) and Claude (via MCP tools). The core principle is **full parity between TUI and MCP** - every state-mutating action flows through the same command bus regardless of source.

## Current Status: Phase 2

Phase 2 implements the grid sequencer:
- 4 tracks: Kick, Snare, Hi-hat, Bass
- 16-step pattern grid
- BPM clock with play/stop
- Command bus architecture
- Event logging for MCP "listening"
- MCP server with transport and pattern tools

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

# Run as MCP server (for Claude integration)
gridoxide --mcp
```

## Controls

| Key | Action |
|-----|--------|
| Arrow keys / hjkl | Navigate grid |
| Space / Enter | Toggle step at cursor |
| P | Play/Stop toggle |
| S | Stop (reset to step 0) |
| +/- | Adjust BPM |
| C | Clear current track |
| F | Fill current track |
| Q / Esc | Quit |

## MCP Tools

When running with `--mcp`, gridoxide exposes these tools:

**Transport:**
- `play` - Start playback
- `stop` - Stop and reset
- `set_bpm` - Set tempo (60-200)
- `get_state` - Get current state

**Pattern:**
- `toggle_step` - Toggle step on/off
- `get_pattern` - Get full grid
- `clear_track` - Clear a track
- `fill_track` - Fill a track

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

## Roadmap

| Phase | Name | Status |
|-------|------|--------|
| 1 | Sound comes out | Complete |
| 2 | Grid sequencer | **Complete** |
| 3 | Sound shaping | Planned |
| 4 | Mixing | Planned |
| 5 | Effects | Planned |
| 6 | Patterns + Arrangement | Planned |
| 7 | Project I/O | Planned |
| 8 | MIDI | Planned |
| 9 | Polish | Planned |

## License

MIT
