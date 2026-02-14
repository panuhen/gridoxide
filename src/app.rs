use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
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
use crate::synth::ParamId;
use crate::ui::{
    get_param_value, render_grid, render_mixer, render_params, render_transport, GridState,
    MixerField, MixerState, ParamEditorState, Theme,
};

/// Current UI view
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Grid,
    Params,
    Mixer,
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
    /// Current view
    view: View,
    /// Whether the app should quit
    should_quit: bool,
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

        Ok(Self {
            theme,
            _audio: audio,
            command_sender,
            event_log,
            sequencer_state,
            grid_state: GridState::new(),
            param_editor: ParamEditorState::new(),
            mixer_state: MixerState::new(),
            view: View::Grid,
            should_quit: false,
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
                        self.handle_key(key.code);
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

    /// Handle key press events
    fn handle_key(&mut self, key: KeyCode) {
        match self.view {
            View::Grid => self.handle_grid_key(key),
            View::Params => self.handle_params_key(key),
            View::Mixer => self.handle_mixer_key(key),
        }
    }

    /// Handle keys in grid view
    fn handle_grid_key(&mut self, key: KeyCode) {
        match key {
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }

            // Switch to params view
            KeyCode::Tab | KeyCode::Char('e') => {
                self.view = View::Params;
                // Sync param editor track with grid cursor
                self.param_editor.switch_track(self.grid_state.cursor_track);
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
                    self.dispatch(Command::Stop);
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
                self.grid_state.move_cursor(-1, 0);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.grid_state.move_cursor(1, 0);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.grid_state.move_cursor(0, -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.grid_state.move_cursor(0, 1);
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

            _ => {}
        }
    }

    /// Handle keys in params view
    fn handle_params_key(&mut self, key: KeyCode) {
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
                self.param_editor.move_selection(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.param_editor.move_selection(1);
            }

            // Switch track
            KeyCode::Char('1') => self.param_editor.switch_track(0),
            KeyCode::Char('2') => self.param_editor.switch_track(1),
            KeyCode::Char('3') => self.param_editor.switch_track(2),
            KeyCode::Char('4') => self.param_editor.switch_track(3),

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
                    self.dispatch(Command::Stop);
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
        match key {
            // Quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Cycle to grid view (Esc also goes back to grid)
            KeyCode::Tab | KeyCode::Esc => {
                self.view = View::Grid;
            }

            // Select track
            KeyCode::Char('1') => self.mixer_state.select_track(0),
            KeyCode::Char('2') => self.mixer_state.select_track(1),
            KeyCode::Char('3') => self.mixer_state.select_track(2),
            KeyCode::Char('4') => self.mixer_state.select_track(3),

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
                    self.dispatch(Command::Stop);
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

    /// Adjust a mixer value based on current field selection
    fn adjust_mixer_value(&mut self, direction: i32) {
        let track = self.mixer_state.selected_track;
        match self.mixer_state.selected_field {
            MixerField::Volume => {
                let current = self.sequencer_state.read().track_volumes[track];
                let new_vol = (current + direction as f32 * 0.05).clamp(0.0, 1.0);
                self.dispatch(Command::SetTrackVolume {
                    track,
                    volume: new_vol,
                });
            }
            MixerField::Pan => {
                let current = self.sequencer_state.read().track_pans[track];
                let new_pan = (current + direction as f32 * 0.1).clamp(-1.0, 1.0);
                self.dispatch(Command::SetTrackPan {
                    track,
                    pan: new_pan,
                });
            }
            MixerField::Mute => {
                self.dispatch(Command::ToggleMute(track));
            }
            MixerField::Solo => {
                self.dispatch(Command::ToggleSolo(track));
            }
        }
    }

    /// Adjust the currently selected parameter
    fn adjust_current_param(&mut self, delta_normalized: f32) {
        let Some(param) = self.param_editor.current_param() else {
            return;
        };

        let (min, max, _default) = param.range();
        let state = self.sequencer_state.read();
        let current = get_param_value(&state, param);
        drop(state);

        let new_value = (current + delta_normalized * (max - min)).clamp(min, max);

        self.dispatch(Command::SetParam {
            param,
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
        render_transport(
            frame,
            chunks[1],
            state.playing,
            state.bpm,
            state.current_step,
            &self.theme,
        );

        // Render main content based on view
        match self.view {
            View::Grid => {
                render_grid(
                    frame,
                    chunks[2],
                    &state.pattern,
                    &self.grid_state,
                    state.current_step,
                    state.playing,
                    &self.theme,
                );
            }
            View::Params => {
                render_params(frame, chunks[2], &state, &self.param_editor, &self.theme);
            }
            View::Mixer => {
                render_mixer(frame, chunks[2], &state, &self.mixer_state, &self.theme);
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

    /// Render the footer with help
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help = match self.view {
            View::Grid => format!(
                "SPACE:Toggle | P:Play | S:Stop | +/-:BPM | C:Clear | F:Fill | TAB:Params | Q:Quit | {}",
                self.theme.name
            ),
            View::Params => format!(
                "1-4:Track | Up/Down:Select | Left/Right:Adjust | [/]:Coarse | TAB:Mixer | Q:Quit | {}",
                self.theme.name
            ),
            View::Mixer => format!(
                "1-4:Track | Up/Down:Field | Left/Right:Adjust | M:Mute | O:Solo | TAB:Grid | Q:Quit | {}",
                self.theme.name
            ),
        };

        let footer = Paragraph::new(help)
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
}
