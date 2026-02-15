use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use parking_lot::RwLock;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use crate::audio::{AudioEngine, SequencerState};
use crate::command::{Command, CommandBus, CommandSender, CommandSource};
use crate::event::EventLog;
use crate::fx::{FilterType, FxParamId, FxType, MasterFxParamId};
use crate::mcp::{start_socket_server, GridoxideMcp};
use crate::project;
use crate::project::renderer::{ExportMode, export_wav};
use crate::sequencer::{PlaybackMode, NUM_PATTERNS};
use crate::ui::{
    get_param_descriptors, get_snapshot_param_value, render_fx, render_grid, render_help,
    render_mixer, render_params, render_song, render_transport, FxEditorState, GridState,
    HelpState, MixerField, MixerState, ParamEditorState, SongState, Theme, TransportInfo,
};
use crate::ui::help::help_line_count;

/// Current UI view
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Grid,
    Params,
    Mixer,
    Fx,
    Song,
    Help,
}

/// Application state
pub struct App {
    /// Current theme
    theme: Theme,
    /// Audio engine
    _audio: AudioEngine,
    /// Command sender for dispatching commands
    command_sender: CommandSender,
    /// Event log for MCP "listening"
    event_log: Arc<RwLock<EventLog>>,
    /// Shared sequencer state (read from audio thread)
    sequencer_state: Arc<RwLock<SequencerState>>,
    /// Grid navigation state
    grid_state: GridState,
    /// Parameter editor state
    param_editor: ParamEditorState,
    /// Mixer state
    mixer_state: MixerState,
    /// FX editor state
    fx_editor: FxEditorState,
    /// Song/arrangement editor state
    song_state: SongState,
    /// Help view state
    help_state: HelpState,
    /// Current view
    view: View,
    /// Previous view (for returning from Help)
    prev_view: View,
    /// Whether the app should quit
    should_quit: bool,
    /// Shutdown flag for the MCP socket server
    mcp_shutdown: Arc<AtomicBool>,
    /// Last project file path (for repeat save/load)
    project_path: Option<PathBuf>,
    /// Temporary status message (e.g., "Saved: project.grox")
    status_message: Option<(String, Instant)>,
}

impl App {
    /// Create a new application with the specified theme
    pub fn new(theme: Theme) -> Result<Self> {
        // Create command bus
        let command_bus = CommandBus::new();
        let command_sender = command_bus.sender();
        let command_receiver = command_bus.receiver();

        // Create audio engine with command receiver
        let audio = AudioEngine::new(command_receiver)?;
        let sequencer_state = audio.state.clone();

        // Create event log
        let event_log = Arc::new(RwLock::new(EventLog::new()));

        // Start MCP socket server (shares same command bus and state as TUI)
        let mcp_shutdown = Arc::new(AtomicBool::new(false));
        let mcp_handler = Arc::new(GridoxideMcp::new(
            command_sender.clone(),
            event_log.clone(),
            sequencer_state.clone(),
        ));
        start_socket_server(mcp_handler, mcp_shutdown.clone());

        Ok(Self {
            theme,
            _audio: audio,
            command_sender,
            event_log,
            sequencer_state,
            grid_state: GridState::new(),
            param_editor: ParamEditorState::new(),
            mixer_state: MixerState::new(),
            fx_editor: FxEditorState::new(),
            song_state: SongState::new(),
            help_state: HelpState::new(),
            view: View::Grid,
            prev_view: View::Grid,
            should_quit: false,
            mcp_shutdown,
            project_path: None,
            status_message: None,
        })
    }

    /// Get a clone of the command sender (for MCP)
    pub fn command_sender(&self) -> CommandSender {
        self.command_sender.clone()
    }

    /// Get a clone of the event log (for MCP)
    pub fn event_log(&self) -> Arc<RwLock<EventLog>> {
        self.event_log.clone()
    }

    /// Get a clone of the sequencer state (for MCP)
    pub fn sequencer_state(&self) -> Arc<RwLock<SequencerState>> {
        self.sequencer_state.clone()
    }

    /// Run the main application loop
    pub fn run(&mut self) -> Result<()> {
        let mut terminal = Self::setup_terminal()?;

        let result = self.main_loop(&mut terminal);

        // Signal socket server to shut down
        self.mcp_shutdown.store(true, Ordering::Relaxed);

        Self::restore_terminal(&mut terminal)?;

        result
    }

    /// Setup the terminal for TUI
    fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(terminal)
    }

    /// Restore terminal to normal state
    fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    /// Main event loop
    fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            // Poll for events with timeout for responsive UI (~60fps)
            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    // Only handle key press events (not release)
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Dispatch a command through the command bus
    fn dispatch(&mut self, cmd: Command) {
        // Log the command
        self.event_log.write().log(cmd.clone(), CommandSource::Tui);
        // Send to audio thread
        self.command_sender.send(cmd, CommandSource::Tui);
    }

    /// Set a temporary status message shown in the footer
    fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Get the current number of tracks
    fn num_tracks(&self) -> usize {
        self.sequencer_state.read().num_tracks()
    }

    /// Handle key press events
    fn handle_key(&mut self, key: KeyEvent) {
        // Global Ctrl keybindings (checked before view-specific)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('s') => {
                    self.save_project_action();
                    return;
                }
                KeyCode::Char('o') => {
                    self.load_project_action();
                    return;
                }
                KeyCode::Char('e') => {
                    self.export_pattern_action();
                    return;
                }
                KeyCode::Char('w') => {
                    self.export_song_action();
                    return;
                }
                _ => {}
            }
        }

        // 'G' toggles Help from any view
        if key.code == KeyCode::Char('g') && self.view != View::Help {
            self.prev_view = self.view;
            self.view = View::Help;
            return;
        }

        match self.view {
            View::Grid => self.handle_grid_key(key.code),
            View::Params => self.handle_params_key(key.code),
            View::Mixer => self.handle_mixer_key(key.code),
            View::Fx => self.handle_fx_key(key.code),
            View::Song => self.handle_song_key(key.code),
            View::Help => self.handle_help_key(key.code),
        }
    }

    fn save_project_action(&mut self) {
        let path = self
            .project_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("project.grox"));
        let state = self.sequencer_state.read().clone();
        match project::save_project(&state, &path) {
            Ok(()) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.set_status(format!("Saved: {}", name));
                self.project_path = Some(path);
            }
            Err(e) => {
                self.set_status(format!("Save failed: {}", e));
            }
        }
    }

    fn load_project_action(&mut self) {
        let path = self
            .project_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("project.grox"));
        match project::load_project(&path) {
            Ok(project_data) => {
                let new_state = project_data.to_state();
                self.dispatch(Command::LoadProject(Box::new(new_state)));
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.set_status(format!("Loaded: {}", name));
                self.project_path = Some(path);
            }
            Err(e) => {
                self.set_status(format!("Load failed: {}", e));
            }
        }
    }

    fn export_pattern_action(&mut self) {
        let state = self.sequencer_state.read().clone();
        let pat_idx = state.current_pattern;
        let filename = format!("pattern_{:02}.wav", pat_idx);
        let path = PathBuf::from(&filename);
        match export_wav(&state, ExportMode::Pattern(pat_idx), &path) {
            Ok(result) => {
                self.set_status(format!("Exported: {} ({:.1}s)", filename, result.duration_secs));
            }
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
            }
        }
    }

    fn export_song_action(&mut self) {
        let state = self.sequencer_state.read().clone();
        let path = PathBuf::from("song.wav");
        match export_wav(&state, ExportMode::Song, &path) {
            Ok(result) => {
                self.set_status(format!("Exported: song.wav ({:.1}s)", result.duration_secs));
            }
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
            }
        }
    }

    /// Handle keys in grid view
    fn handle_grid_key(&mut self, key: KeyCode) {
        let num_tracks = self.num_tracks();
        match key {
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }

            // Switch to params view
            KeyCode::Tab | KeyCode::Char('e') => {
                self.view = View::Params;
                // Sync param editor track with grid cursor
                self.param_editor.switch_track(self.grid_state.cursor_track, num_tracks);
            }

            // Toggle step at cursor
            KeyCode::Char(' ') | KeyCode::Enter => {
                let cmd = Command::ToggleStep {
                    track: self.grid_state.cursor_track,
                    step: self.grid_state.cursor_step,
                };
                self.dispatch(cmd);
            }

            // Play/Pause toggle
            KeyCode::Char('p') => {
                let playing = self.sequencer_state.read().playing;
                if playing {
                    self.dispatch(Command::Pause);
                } else {
                    self.dispatch(Command::Play);
                }
            }

            // Stop (reset to beginning)
            KeyCode::Char('s') => {
                self.dispatch(Command::Stop);
            }

            // Navigation
            KeyCode::Left | KeyCode::Char('h') => {
                self.grid_state.move_cursor(-1, 0, num_tracks);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.grid_state.move_cursor(1, 0, num_tracks);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.grid_state.move_cursor(0, -1, num_tracks);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.grid_state.move_cursor(0, 1, num_tracks);
            }

            // BPM control
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let current_bpm = self.sequencer_state.read().bpm;
                self.dispatch(Command::SetBpm(current_bpm + 5.0));
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let current_bpm = self.sequencer_state.read().bpm;
                self.dispatch(Command::SetBpm(current_bpm - 5.0));
            }

            // Clear current track
            KeyCode::Char('c') => {
                self.dispatch(Command::ClearTrack(self.grid_state.cursor_track));
            }

            // Fill current track
            KeyCode::Char('f') => {
                self.dispatch(Command::FillTrack(self.grid_state.cursor_track));
            }

            // Note down 1 semitone
            KeyCode::Char('[') => {
                self.adjust_step_note(-1);
            }
            // Note up 1 semitone
            KeyCode::Char(']') => {
                self.adjust_step_note(1);
            }
            // Note down 1 octave (Shift+[)
            KeyCode::Char('{') => {
                self.adjust_step_note(-12);
            }
            // Note up 1 octave (Shift+])
            KeyCode::Char('}') => {
                self.adjust_step_note(12);
            }

            // Pattern selection
            KeyCode::Char(',') | KeyCode::Char('<') => {
                let current = self.sequencer_state.read().current_pattern;
                let new_pat = if current == 0 { NUM_PATTERNS - 1 } else { current - 1 };
                self.dispatch(Command::SelectPattern(new_pat));
            }
            KeyCode::Char('.') | KeyCode::Char('>') => {
                let current = self.sequencer_state.read().current_pattern;
                let new_pat = (current + 1) % NUM_PATTERNS;
                self.dispatch(Command::SelectPattern(new_pat));
            }

            _ => {}
        }
    }

    /// Handle keys in params view
    fn handle_params_key(&mut self, key: KeyCode) {
        let num_tracks = self.num_tracks();
        let param_count = {
            let state = self.sequencer_state.read();
            get_param_descriptors(&state, self.param_editor.track).len()
        };

        match key {
            // Quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Cycle to mixer view (Esc goes back to grid)
            KeyCode::Tab => {
                self.view = View::Mixer;
            }
            KeyCode::Esc => {
                self.view = View::Grid;
            }

            // Navigate params
            KeyCode::Up | KeyCode::Char('k') => {
                self.param_editor.move_selection(-1, param_count);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.param_editor.move_selection(1, param_count);
            }

            // Switch track (1-9)
            KeyCode::Char(c @ '1'..='9') => {
                let track = (c as usize) - ('1' as usize);
                self.param_editor.switch_track(track, num_tracks);
            }

            // Adjust value (fine)
            KeyCode::Left | KeyCode::Char('h') => {
                self.adjust_current_param(-0.05);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.adjust_current_param(0.05);
            }

            // Adjust value (coarse)
            KeyCode::Char('[') => {
                self.adjust_current_param(-0.2);
            }
            KeyCode::Char(']') => {
                self.adjust_current_param(0.2);
            }

            // Play/Stop still works in params view
            KeyCode::Char('p') => {
                let playing = self.sequencer_state.read().playing;
                if playing {
                    self.dispatch(Command::Pause);
                } else {
                    self.dispatch(Command::Play);
                }
            }

            KeyCode::Char('s') => {
                self.dispatch(Command::Stop);
            }

            _ => {}
        }
    }

    /// Handle keys in mixer view
    fn handle_mixer_key(&mut self, key: KeyCode) {
        let num_tracks = self.num_tracks();
        match key {
            // Quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Tab cycles to FX view, Esc goes back to grid
            KeyCode::Tab => {
                self.view = View::Fx;
            }
            KeyCode::Esc => {
                self.view = View::Grid;
            }

            // Select track (1-9)
            KeyCode::Char(c @ '1'..='9') => {
                let track = (c as usize) - ('1' as usize);
                self.mixer_state.select_track(track, num_tracks);
            }

            // Navigate fields
            KeyCode::Up | KeyCode::Char('k') => {
                self.mixer_state.move_field(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.mixer_state.move_field(1);
            }

            // Adjust value or toggle
            KeyCode::Left | KeyCode::Char('h') => {
                self.adjust_mixer_value(-1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.adjust_mixer_value(1);
            }

            // Toggle mute on selected track
            KeyCode::Char('m') => {
                self.dispatch(Command::ToggleMute(self.mixer_state.selected_track));
            }

            // Toggle solo on selected track
            KeyCode::Char('o') => {
                self.dispatch(Command::ToggleSolo(self.mixer_state.selected_track));
            }

            // Play/Stop
            KeyCode::Char('p') => {
                let playing = self.sequencer_state.read().playing;
                if playing {
                    self.dispatch(Command::Pause);
                } else {
                    self.dispatch(Command::Play);
                }
            }
            KeyCode::Char('s') => {
                self.dispatch(Command::Stop);
            }

            _ => {}
        }
    }

    /// Handle keys in FX view
    fn handle_fx_key(&mut self, key: KeyCode) {
        let num_tracks = self.num_tracks();
        match key {
            // Quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Tab cycles to Song view, Esc goes back to grid
            KeyCode::Tab => {
                self.view = View::Song;
            }
            KeyCode::Esc => {
                self.view = View::Grid;
            }

            // Select track (1-9) or master (m)
            KeyCode::Char(c @ '1'..='9') => {
                let track = (c as usize) - ('1' as usize);
                self.fx_editor.select_track(track, num_tracks);
            }
            KeyCode::Char('m') => {
                self.fx_editor.select_track(num_tracks, num_tracks);
            }

            // Navigate params
            KeyCode::Up | KeyCode::Char('k') => {
                self.fx_editor.move_selection(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.fx_editor.move_selection(1);
            }

            // Adjust value (fine)
            KeyCode::Left | KeyCode::Char('h') => {
                self.adjust_fx_param(-0.05);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.adjust_fx_param(0.05);
            }

            // Adjust value (coarse)
            KeyCode::Char('[') => {
                self.adjust_fx_param(-0.2);
            }
            KeyCode::Char(']') => {
                self.adjust_fx_param(0.2);
            }

            // Toggle effect enabled (Enter/Space)
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle_current_fx();
            }

            // Play/Stop
            KeyCode::Char('p') => {
                let playing = self.sequencer_state.read().playing;
                if playing {
                    self.dispatch(Command::Pause);
                } else {
                    self.dispatch(Command::Play);
                }
            }
            KeyCode::Char('s') => {
                self.dispatch(Command::Stop);
            }

            _ => {}
        }
    }

    /// Handle keys in song/arrangement view
    fn handle_song_key(&mut self, key: KeyCode) {
        match key {
            // Quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Tab cycles to Grid, Esc goes back to grid
            KeyCode::Tab => {
                self.view = View::Grid;
            }
            KeyCode::Esc => {
                self.view = View::Grid;
            }

            // Navigate arrangement
            KeyCode::Up | KeyCode::Char('k') => {
                let arr_len = self.sequencer_state.read().arrangement.len();
                if arr_len > 0 && self.song_state.cursor_position > 0 {
                    self.song_state.cursor_position -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let arr_len = self.sequencer_state.read().arrangement.len();
                if arr_len > 0 && self.song_state.cursor_position < arr_len - 1 {
                    self.song_state.cursor_position += 1;
                }
            }

            // Adjust repeat count
            KeyCode::Left | KeyCode::Char('h') => {
                let state = self.sequencer_state.read();
                let pos = self.song_state.cursor_position;
                if pos < state.arrangement.len() {
                    let entry = state.arrangement.entries[pos];
                    drop(state);
                    if entry.repeats > 1 {
                        self.dispatch(Command::SetArrangementEntry {
                            position: pos,
                            pattern: entry.pattern,
                            repeats: entry.repeats - 1,
                        });
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let state = self.sequencer_state.read();
                let pos = self.song_state.cursor_position;
                if pos < state.arrangement.len() {
                    let entry = state.arrangement.entries[pos];
                    drop(state);
                    if entry.repeats < 16 {
                        self.dispatch(Command::SetArrangementEntry {
                            position: pos,
                            pattern: entry.pattern,
                            repeats: entry.repeats + 1,
                        });
                    }
                }
            }

            // Cycle pattern index on selected entry
            KeyCode::Char('-') => {
                let state = self.sequencer_state.read();
                let pos = self.song_state.cursor_position;
                if pos < state.arrangement.len() {
                    let entry = state.arrangement.entries[pos];
                    drop(state);
                    let new_pat = if entry.pattern == 0 { NUM_PATTERNS - 1 } else { entry.pattern - 1 };
                    self.dispatch(Command::SetArrangementEntry {
                        position: pos,
                        pattern: new_pat,
                        repeats: entry.repeats,
                    });
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let state = self.sequencer_state.read();
                let pos = self.song_state.cursor_position;
                if pos < state.arrangement.len() {
                    let entry = state.arrangement.entries[pos];
                    drop(state);
                    let new_pat = (entry.pattern + 1) % NUM_PATTERNS;
                    self.dispatch(Command::SetArrangementEntry {
                        position: pos,
                        pattern: new_pat,
                        repeats: entry.repeats,
                    });
                }
            }

            // Append current pattern to arrangement
            KeyCode::Char('a') => {
                let current_pat = self.sequencer_state.read().current_pattern;
                self.dispatch(Command::AppendArrangement {
                    pattern: current_pat,
                    repeats: 1,
                });
                // Move cursor to new entry
                let new_len = self.sequencer_state.read().arrangement.len();
                if new_len > 0 {
                    self.song_state.cursor_position = new_len - 1;
                }
            }

            // Delete entry at cursor
            KeyCode::Char('d') | KeyCode::Delete => {
                let arr_len = self.sequencer_state.read().arrangement.len();
                if arr_len > 0 {
                    self.dispatch(Command::RemoveArrangement(self.song_state.cursor_position));
                    // Adjust cursor
                    let new_len = self.sequencer_state.read().arrangement.len();
                    if self.song_state.cursor_position >= new_len && new_len > 0 {
                        self.song_state.cursor_position = new_len - 1;
                    }
                }
            }

            // Set entry's pattern to current pattern
            KeyCode::Enter => {
                let state = self.sequencer_state.read();
                let pos = self.song_state.cursor_position;
                if pos < state.arrangement.len() {
                    let current_pat = state.current_pattern;
                    let repeats = state.arrangement.entries[pos].repeats;
                    drop(state);
                    self.dispatch(Command::SetArrangementEntry {
                        position: pos,
                        pattern: current_pat,
                        repeats,
                    });
                }
            }

            // Pattern selection (same as grid)
            KeyCode::Char(',') | KeyCode::Char('<') => {
                let current = self.sequencer_state.read().current_pattern;
                let new_pat = if current == 0 { NUM_PATTERNS - 1 } else { current - 1 };
                self.dispatch(Command::SelectPattern(new_pat));
            }
            KeyCode::Char('.') | KeyCode::Char('>') => {
                let current = self.sequencer_state.read().current_pattern;
                let new_pat = (current + 1) % NUM_PATTERNS;
                self.dispatch(Command::SelectPattern(new_pat));
            }

            // Toggle Pattern/Song mode
            KeyCode::Char('m') => {
                let current_mode = self.sequencer_state.read().playback_mode;
                let new_mode = match current_mode {
                    PlaybackMode::Pattern => PlaybackMode::Song,
                    PlaybackMode::Song => PlaybackMode::Pattern,
                };
                self.dispatch(Command::SetPlaybackMode(new_mode));
            }

            // Copy current pattern to next empty slot (or prompt)
            KeyCode::Char('c') => {
                let state = self.sequencer_state.read();
                let src = state.current_pattern;
                // Find next empty slot
                let dst = (0..NUM_PATTERNS)
                    .find(|&i| i != src && !state.pattern_bank.has_content(i));
                drop(state);
                if let Some(dst) = dst {
                    self.dispatch(Command::CopyPattern { src, dst });
                }
            }

            // Clear current pattern slot
            KeyCode::Char('x') => {
                let current = self.sequencer_state.read().current_pattern;
                self.dispatch(Command::ClearPattern(current));
            }

            // Play/Stop
            KeyCode::Char('p') => {
                let playing = self.sequencer_state.read().playing;
                if playing {
                    self.dispatch(Command::Pause);
                } else {
                    self.dispatch(Command::Play);
                }
            }
            KeyCode::Char('s') => {
                self.dispatch(Command::Stop);
            }

            _ => {}
        }
    }

    /// Handle keys in help view
    fn handle_help_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('g') | KeyCode::Tab => {
                self.view = self.prev_view;
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_state.scroll_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let total = help_line_count(&self.theme);
                // Rough estimate of visible lines
                self.help_state.scroll_down(total, 20);
            }
            _ => {}
        }
    }

    /// Toggle the FX effect that the cursor is currently in
    fn toggle_current_fx(&mut self) {
        let num_tracks = self.num_tracks();
        if self.fx_editor.is_master(num_tracks) {
            // Master: toggle reverb
            self.dispatch(Command::ToggleMasterFxEnabled);
        } else {
            let track = self.fx_editor.track;
            let (section, _) = self.fx_editor.current_section_and_param();
            let fx = match section {
                0 => FxType::Filter,
                1 => FxType::Distortion,
                2 => FxType::Delay,
                _ => return,
            };
            self.dispatch(Command::ToggleFxEnabled { track, fx });
        }
    }

    /// Adjust the currently selected FX parameter
    fn adjust_fx_param(&mut self, delta_normalized: f32) {
        let num_tracks = self.num_tracks();
        if self.fx_editor.is_master(num_tracks) {
            // Master FX
            let params = MasterFxParamId::all();
            if self.fx_editor.param_index >= params.len() {
                return;
            }
            let param = params[self.fx_editor.param_index];
            let (min, max, _default) = param.range();
            let current = crate::ui::fx::get_master_fx_param_value(
                &self.sequencer_state.read(),
                param,
            );
            let new_value = (current + delta_normalized * (max - min)).clamp(min, max);
            self.dispatch(Command::SetMasterFxParam {
                param,
                value: new_value,
            });
        } else {
            let track = self.fx_editor.track;
            let (section, local_idx) = self.fx_editor.current_section_and_param();

            // Filter type is special: cycle through LP/HP/BP
            if section == 0 && local_idx == 0 {
                let state = self.sequencer_state.read();
                if track < state.tracks.len() {
                    let current_type = state.tracks[track].fx.filter_type;
                    drop(state);
                    let dir = if delta_normalized > 0.0 { 1i32 } else { -1i32 };
                    let new_idx = (current_type.index() as i32 + dir).rem_euclid(3) as usize;
                    let new_type = FilterType::from_index(new_idx);
                    self.dispatch(Command::SetFxFilterType {
                        track,
                        filter_type: new_type,
                    });
                }
                return;
            }

            // Map (section, local_idx) to FxParamId
            let param = match (section, local_idx) {
                (0, 1) => FxParamId::FilterCutoff,
                (0, 2) => FxParamId::FilterResonance,
                (1, 0) => FxParamId::DistDrive,
                (1, 1) => FxParamId::DistMix,
                (2, 0) => FxParamId::DelayTime,
                (2, 1) => FxParamId::DelayFeedback,
                (2, 2) => FxParamId::DelayMix,
                _ => return,
            };

            let (min, max, _default) = param.range();
            let current = crate::ui::fx::get_fx_param_value(
                &self.sequencer_state.read(),
                track,
                param,
            );
            let new_value = (current + delta_normalized * (max - min)).clamp(min, max);
            self.dispatch(Command::SetFxParam {
                track,
                param,
                value: new_value,
            });
        }
    }

    /// Adjust a mixer value based on current field selection
    fn adjust_mixer_value(&mut self, direction: i32) {
        let track = self.mixer_state.selected_track;
        let state = self.sequencer_state.read();
        if track >= state.tracks.len() {
            return;
        }
        match self.mixer_state.selected_field {
            MixerField::Volume => {
                let current = state.tracks[track].volume;
                drop(state);
                let new_vol = (current + direction as f32 * 0.05).clamp(0.0, 1.0);
                self.dispatch(Command::SetTrackVolume {
                    track,
                    volume: new_vol,
                });
            }
            MixerField::Pan => {
                let current = state.tracks[track].pan;
                drop(state);
                let new_pan = (current + direction as f32 * 0.1).clamp(-1.0, 1.0);
                self.dispatch(Command::SetTrackPan {
                    track,
                    pan: new_pan,
                });
            }
            MixerField::Mute => {
                drop(state);
                self.dispatch(Command::ToggleMute(track));
            }
            MixerField::Solo => {
                drop(state);
                self.dispatch(Command::ToggleSolo(track));
            }
        }
    }

    /// Adjust the note of the current step in grid view (semitone delta)
    fn adjust_step_note(&mut self, delta: i32) {
        let track = self.grid_state.cursor_track;
        let step = self.grid_state.cursor_step;
        let state = self.sequencer_state.read();
        let step_data = state.pattern.get_step(track, step);
        drop(state);

        // Only adjust note on active steps
        if !step_data.active {
            return;
        }

        let new_note = (step_data.note as i32 + delta).clamp(0, 127) as u8;
        self.dispatch(Command::SetStepNote {
            track,
            step,
            note: new_note,
        });
    }

    /// Adjust the currently selected parameter (uses string-key system)
    fn adjust_current_param(&mut self, delta_normalized: f32) {
        let track = self.param_editor.track;
        let idx = self.param_editor.param_index;

        let state = self.sequencer_state.read();
        let descriptors = get_param_descriptors(&state, track);
        if idx >= descriptors.len() {
            return;
        }

        let desc = &descriptors[idx];
        let current = get_snapshot_param_value(&state, track, &desc.key);
        drop(state);

        let range = desc.max - desc.min;
        let new_value = (current + delta_normalized * range).clamp(desc.min, desc.max);

        self.dispatch(Command::SetTrackParam {
            track,
            key: desc.key.clone(),
            value: new_value,
        });
    }

    /// Render the UI
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Clear with background color
        let bg_block = Block::default().style(Style::default().bg(self.theme.bg));
        frame.render_widget(bg_block, area);

        // Layout: header, transport, main content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(3), // Transport
                Constraint::Min(6),    // Main content (grid or params)
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Get current state
        let state = self.sequencer_state.read();

        self.render_header(frame, chunks[0]);
        // Get cursor note info for transport display
        let cursor_note = {
            let step_data = state.pattern.get_step(
                self.grid_state.cursor_track,
                self.grid_state.cursor_step,
            );
            if self.view == View::Grid {
                Some((step_data.active, step_data.note))
            } else {
                None
            }
        };
        let transport_info = TransportInfo {
            playing: state.playing,
            bpm: state.bpm,
            current_step: state.current_step,
            current_pattern: state.current_pattern,
            playback_mode: state.playback_mode,
            arrangement_position: state.arrangement_position,
            arrangement_len: state.arrangement.len(),
            cursor_note,
            pending_pattern: None,
        };
        render_transport(
            frame,
            chunks[1],
            &transport_info,
            &self.theme,
        );

        // Render main content based on view
        match self.view {
            View::Grid => {
                let track_names: Vec<String> = state.tracks.iter().map(|t| t.name.clone()).collect();
                render_grid(
                    frame,
                    chunks[2],
                    &state.pattern,
                    &self.grid_state,
                    state.current_step,
                    state.playing,
                    &track_names,
                    &self.theme,
                );
            }
            View::Params => {
                render_params(frame, chunks[2], &state, &self.param_editor, &self.theme);
            }
            View::Mixer => {
                render_mixer(frame, chunks[2], &state, &self.mixer_state, &self.theme);
            }
            View::Fx => {
                render_fx(frame, chunks[2], &state, &self.fx_editor, &self.theme);
            }
            View::Song => {
                render_song(frame, chunks[2], &state, &self.song_state, &self.theme);
            }
            View::Help => {
                drop(state);
                render_help(frame, chunks[2], &self.help_state, &self.theme);
            }
        }

        self.render_footer(frame, chunks[3]);
    }

    /// Render the header
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let view_indicator = match self.view {
            View::Grid => "[GRID]",
            View::Params => "[PARAMS]",
            View::Mixer => "[MIXER]",
            View::Fx => "[FX]",
            View::Song => "[SONG]",
            View::Help => "[HELP]",
        };
        let title = format!(
            " GRIDOXIDE v{} {} ",
            env!("CARGO_PKG_VERSION"),
            view_indicator
        );
        let header = Paragraph::new(title)
            .style(
                Style::default()
                    .fg(self.theme.highlight)
                    .bg(self.theme.bg)
                    .bold(),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.border))
                    .style(Style::default().bg(self.theme.bg)),
            );
        frame.render_widget(header, area);
    }

    /// Render the footer with help or status message
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        // Show status message if recent (within 3 seconds)
        let text = if let Some((ref msg, instant)) = self.status_message {
            if instant.elapsed().as_secs() < 3 {
                msg.clone()
            } else {
                self.footer_help()
            }
        } else {
            self.footer_help()
        };

        let footer = Paragraph::new(text)
            .style(Style::default().fg(self.theme.dimmed).bg(self.theme.bg))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.border))
                    .style(Style::default().bg(self.theme.bg)),
            );
        frame.render_widget(footer, area);
    }

    fn footer_help(&self) -> String {
        match self.view {
            View::Grid => format!(
                "SPACE:Toggle | [/]:Note | ,/.:Pattern | P:Play | S:Stop | C-s:Save | C-o:Load | G:Help | TAB:Params | Q:Quit | {}",
                self.theme.name
            ),
            View::Params => format!(
                "1-9:Track | Up/Down:Select | Left/Right:Adjust | [/]:Coarse | C-s:Save | G:Help | TAB:Mixer | Q:Quit | {}",
                self.theme.name
            ),
            View::Mixer => format!(
                "1-9:Track | Up/Down:Field | Left/Right:Adjust | M:Mute | O:Solo | C-s:Save | G:Help | TAB:FX | Q:Quit | {}",
                self.theme.name
            ),
            View::Fx => format!(
                "1-9:Track | M:Master | Up/Down:Select | Left/Right:Adjust | SPACE:Toggle FX | G:Help | TAB:Song | Q:Quit | {}",
                self.theme.name
            ),
            View::Song => format!(
                "Up/Down:Move | Left/Right:Repeats | +/-:Pattern | A:Add | D:Delete | M:Mode | G:Help | TAB:Grid | Q:Quit | {}",
                self.theme.name
            ),
            View::Help => format!(
                "Up/Down:Scroll | G/Esc/Tab:Back | Q:Quit | {}",
                self.theme.name
            ),
        }
    }
}
