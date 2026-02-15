# gridoxide — Terminal EDM Production Studio

## Project Overview

A Rust-based terminal step sequencer and synthesizer for EDM production. Two users interact with the same system:
- **Human**: TUI with keyboard controls
- **Claude**: MCP tools

Core principle: **full parity between TUI and MCP** via unified command bus. Every state-mutating action flows through the same path regardless of source.

---

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│                     TUI (ratatui)                            │
└──────────────────────────┬──────────────────────────────────┘
                           │ commands
┌──────────────────────────▼──────────────────────────────────┐
│                    Command Bus                               │
│  TUI (keypress) ──┐                                         │
│                   ├──▶ Command ──▶ Engine ──▶ Event Log     │
│  MCP (tool call) ─┘                                         │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Audio Engine (cpal + fundsp)              │
└─────────────────────────────────────────────────────────────┘
```

**Event Log**: All commands are logged with source (tui/mcp), enabling Claude to "listen" to what the human is doing by querying recent events.

---

## Phase Roadmap

| Phase | Name | Core Deliverable | Status |
|-------|------|------------------|--------|
| 1 | Sound comes out | Audio output, basic synth, key triggers | **COMPLETE** |
| 2 | Grid sequencer | Step grid, clock, patterns, play/stop, MCP transport+pattern | **COMPLETE** |
| 3 | Sound shaping | Synth parameters, TUI editor, MCP track control | **COMPLETE** |
| 4 | Mixing | Volume, pan, mute/solo, mixer view, MCP mixer | **COMPLETE** |
| 4.5 | Per-step notes | Per-step pitch/note data, melodic sequencing, MCP note tools, TUI+MCP socket bridge | **COMPLETE** |
| 5 | Effects | FX chains, filter/delay/reverb/distortion, MCP fx | **COMPLETE** |
| 6 | Patterns + Arrangement | Multiple patterns, chaining, arrangement view | **COMPLETE** |
| 7 | Project I/O | Save/load JSON, export WAV | **COMPLETE** |
| 8 | Sample Engine | Load .wav samples per track, pitch shifting, start/end points | Planned |
| 9 | Polish | Velocity/probability, undo/redo, more engines | Planned |

---

## Theme System

**Approach**: Terminal-native as default (inherits user's terminal theme), plus built-in themes.

**Built-in themes**:
- `default` — uses terminal's ANSI colors (inherits Ghostty/etc)
- `phosphor-green` — classic green CRT
- `amber-crt` — warm amber monochrome
- `blue-terminal` — cool blue tones
- `high-contrast` — stark black/white

**Theme struct**:
```rust
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub grid_active: Color,
    pub grid_inactive: Color,
    pub grid_cursor: Color,
    pub track_label: Color,
    pub meter_low: Color,
    pub meter_mid: Color,
    pub meter_high: Color,
    pub border: Color,
    pub highlight: Color,
    pub dimmed: Color,
}
```

**NOT MCP-controllable** — theme is a user preference, not a production parameter.

Theme selection: command-line flag `--theme phosphor-green` or config file.

---

## Phase 1: Sound Comes Out (COMPLETE)

### Goal
Prove the audio pipeline works. Press a key, hear a sound.

### Deliverables
1. ✅ Cargo project with dependencies
2. ✅ Audio output via cpal
3. ✅ Basic kick drum synth
4. ✅ TUI scaffold with ratatui
5. ✅ Keypress triggers sound
6. ✅ Theme system foundation
7. ✅ Project documentation in repo

### File Structure

```
gridoxide/
├── Cargo.toml
├── README.md
├── PLAN.md
├── src/
│   ├── main.rs                # Entry point, arg parsing
│   ├── app.rs                 # App state, main loop
│   ├── audio/
│   │   ├── mod.rs
│   │   └── engine.rs          # cpal setup, audio callback
│   ├── synth/
│   │   ├── mod.rs
│   │   └── kick.rs            # Basic kick drum
│   └── ui/
│       ├── mod.rs
│       └── theme.rs           # Theme definitions
```

### Controls
- **Space**: Trigger kick drum
- **Q / Esc**: Quit

### Verification
1. ✅ `cargo build --release` succeeds
2. ✅ `./target/release/gridoxide` opens TUI
3. ✅ Press spacebar → hear kick drum
4. ✅ Press Q → clean exit
5. ✅ `--theme phosphor-green` changes colors

---

## MCP Integration (Phase 2+)

MCP server will expose 6 tools + event query:

| Tool | Purpose |
|------|---------|
| `transport` | play, stop, bpm, swing |
| `pattern` | toggle steps, get/set patterns, fill, shift |
| `track` | list tracks, synth params |
| `mixer` | volume, pan, mute, solo |
| `fx` | effect chains |
| `arrangement` | pattern chaining, project I/O |

Plus: `get_events(since_id)` — returns command log for Claude to "listen"

---

## Event Log Design

```rust
pub struct Event {
    pub id: u64,
    pub timestamp: u64,
    pub source: EventSource,  // Tui, Mcp
    pub command: Command,
}

pub struct EventLog {
    events: VecDeque<Event>,  // Ring buffer, ~500 events
    next_id: u64,
}
```

Logged: all state-mutating commands
Not logged: navigation, view switching (TUI-only noise)

---

## Phase 2: Grid Sequencer (COMPLETE)

### Goal
Build a functional step sequencer with clock, multiple tracks, and the command bus architecture that enables MCP integration.

### Deliverables
1. ✅ Step pattern data structure (16 steps × 4 tracks)
2. ✅ Clock/BPM timing with play/stop
3. ✅ Grid view with cursor navigation
4. ✅ 4 basic sounds: kick, snare, hihat, bass
5. ✅ Command bus architecture (TUI and MCP use same path)
6. ✅ Event logging system
7. ✅ MCP server with transport + pattern tools

### File Structure (Phase 2)

```
gridoxide/
├── Cargo.toml
├── README.md
├── PLAN.md
├── src/
│   ├── main.rs
│   ├── app.rs                 # Updated with grid view
│   ├── audio/
│   │   ├── mod.rs
│   │   └── engine.rs          # Updated for multiple synths
│   ├── synth/
│   │   ├── mod.rs
│   │   ├── kick.rs
│   │   ├── snare.rs           # NEW
│   │   ├── hihat.rs           # NEW
│   │   └── bass.rs            # NEW
│   ├── sequencer/             # NEW
│   │   ├── mod.rs
│   │   ├── pattern.rs         # Step pattern data
│   │   └── clock.rs           # BPM timing
│   ├── command/               # NEW
│   │   ├── mod.rs
│   │   ├── bus.rs             # Command dispatcher
│   │   └── types.rs           # Command enum
│   ├── event/                 # NEW
│   │   ├── mod.rs
│   │   └── log.rs             # Event log ring buffer
│   ├── mcp/                   # NEW
│   │   ├── mod.rs
│   │   └── server.rs          # MCP tool handlers
│   └── ui/
│       ├── mod.rs
│       ├── theme.rs
│       └── grid.rs            # NEW - grid rendering
```

### Controls (Phase 2)
- **Arrow keys**: Navigate grid cursor
- **Space/Enter**: Toggle step at cursor
- **P**: Play/Pause
- **S**: Stop (reset to step 0)
- **+/-**: Adjust BPM
- **1-4**: Select track
- **Q/Esc**: Quit

### Command Bus Design

```rust
pub enum Command {
    // Transport
    Play,
    Stop,
    SetBpm(f32),

    // Pattern
    ToggleStep { track: usize, step: usize },
    ClearTrack(usize),
    FillTrack(usize),

    // Navigation (TUI only, not logged)
    MoveCursor { dx: i32, dy: i32 },
}

pub enum CommandSource {
    Tui,
    Mcp,
}
```

### MCP Tools (Phase 2)

**transport**
- `play()` - Start playback
- `stop()` - Stop playback
- `get_state()` - Returns { playing, bpm, current_step }
- `set_bpm(bpm)` - Set tempo

**pattern**
- `toggle_step(track, step)` - Toggle a step
- `get_pattern()` - Returns full grid state
- `clear_track(track)` - Clear all steps on track
- `fill_track(track)` - Fill all steps on track

**events**
- `get_events(since_id)` - Get recent events for "listening"

### Verification (Phase 2)
1. ✅ `cargo build --release` succeeds
2. ✅ `cargo install --path .` installs to ~/.cargo/bin
3. ✅ Run `gridoxide` from anywhere
4. ✅ Grid displays 4 tracks × 16 steps
5. ✅ Arrow keys move cursor, Space toggles steps
6. ✅ P starts playback, hear pattern loop
7. ✅ +/- adjusts BPM visually and audibly
8. ✅ MCP tools available via `gridoxide --mcp`

### Installation
```bash
cd /home/panu/gridoxide
cargo install --path .
# Now run from anywhere:
gridoxide
gridoxide --theme phosphor-green
```

---

## Phase 3: Sound Shaping (COMPLETE)

### Goal
Add synthesizer parameter controls to shape the sound of each track. Both TUI and MCP can modify parameters in real-time.

### Deliverables
1. ✅ Parameter structs for each synth (KickParams, SnareParams, HiHatParams, BassParams)
2. ✅ ParamId enum for addressing individual parameters
3. ✅ Real-time parameter updates in audio thread
4. ✅ TUI parameter editor view (Tab to switch between Grid and Params views)
5. ✅ MCP track parameter tools (list_tracks, get_track_params, set_param, reset_track)
6. ✅ Parameter ranges with min/max/default values

### File Structure (Phase 3 additions)

```
gridoxide/
├── src/
│   ├── synth/
│   │   ├── params.rs           # NEW - parameter structs and ParamId
│   │   └── ... (updated synths with params)
│   ├── ui/
│   │   └── params.rs           # NEW - parameter editor view
│   └── mcp/
│       └── server.rs           # Updated with track parameter tools
```

### Controls (Phase 3 additions)

**Grid View**
- **Tab / E**: Switch to Params view

**Params View**
- **Tab / Esc**: Switch back to Grid view
- **1-4**: Select track
- **Up/Down or J/K**: Select parameter
- **Left/Right or H/L**: Adjust value (fine: ±5%)
- **[ / ]**: Adjust value (coarse: ±20%)
- **P**: Play/Pause (still works)
- **S**: Stop (still works)
- **Q**: Quit

### Synth Parameters

**Kick (Track 0)**
| Parameter | Key | Range | Default |
|-----------|-----|-------|---------|
| Pitch Start | kick_pitch_start | 80-250 Hz | 150 |
| Pitch End | kick_pitch_end | 30-80 Hz | 50 |
| Pitch Decay | kick_pitch_decay | 4-20 | 8 |
| Amp Decay | kick_amp_decay | 5-20 | 10 |
| Click | kick_click | 0-1 | 0.3 |
| Drive | kick_drive | 0-1 | 0 |

**Snare (Track 1)**
| Parameter | Key | Range | Default |
|-----------|-----|-------|---------|
| Tone Freq | snare_tone_freq | 120-300 Hz | 180 |
| Tone Decay | snare_tone_decay | 10-40 | 20 |
| Noise Decay | snare_noise_decay | 8-30 | 15 |
| Tone Mix | snare_tone_mix | 0-1 | 0.4 |
| Snappy | snare_snappy | 0-1 | 0.6 |

**HiHat (Track 2)**
| Parameter | Key | Range | Default |
|-----------|-----|-------|---------|
| Decay | hihat_decay | 20-100 | 40 |
| Tone | hihat_tone | 0-1 | 0.5 |
| Open | hihat_open | 0-1 | 0 |

**Bass (Track 3)**
| Parameter | Key | Range | Default |
|-----------|-----|-------|---------|
| Frequency | bass_frequency | 30-120 Hz | 55 |
| Decay | bass_decay | 3-12 | 6 |
| Saw Mix | bass_saw_mix | 0-1 | 0.2 |
| Sub | bass_sub | 0-1 | 0 |

### MCP Tools (Phase 3 additions)

**track**
- `list_tracks()` - List all tracks with available parameters
- `get_track_params(track)` - Get all params for a track with current values
- `set_param(param, value)` - Set a single parameter by key
- `reset_track(track)` - Reset track to default parameters

### Verification (Phase 3)
1. ✅ `cargo build --release` succeeds
2. ✅ `cargo install --path .` installs v0.3.0
3. ✅ Tab switches between Grid and Params views
4. ✅ Parameter values can be adjusted in real-time
5. ✅ Sound changes respond immediately to parameter tweaks
6. ✅ MCP track tools available and functional
