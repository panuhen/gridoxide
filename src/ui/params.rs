use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::audio::SequencerState;
use crate::sequencer::TrackType;
use crate::synth::ParamId;
use crate::ui::Theme;

/// State for parameter editor view
pub struct ParamEditorState {
    pub track: usize,
    pub param_index: usize,
}

impl ParamEditorState {
    pub fn new() -> Self {
        Self {
            track: 0,
            param_index: 0,
        }
    }

    /// Get currently selected parameter
    pub fn current_param(&self) -> Option<ParamId> {
        let params = ParamId::params_for_track(self.track);
        params.get(self.param_index).copied()
    }

    /// Move parameter selection up/down
    pub fn move_selection(&mut self, dy: i32) {
        let params = ParamId::params_for_track(self.track);
        if params.is_empty() {
            return;
        }
        let len = params.len() as i32;
        self.param_index = ((self.param_index as i32 + dy).rem_euclid(len)) as usize;
    }

    /// Switch to a different track
    pub fn switch_track(&mut self, track: usize) {
        if track < 4 {
            self.track = track;
            self.param_index = 0;
        }
    }
}

impl Default for ParamEditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the current value of a parameter from state
pub fn get_param_value(state: &SequencerState, param: ParamId) -> f32 {
    match param {
        ParamId::KickPitchStart => state.kick_params.pitch_start,
        ParamId::KickPitchEnd => state.kick_params.pitch_end,
        ParamId::KickPitchDecay => state.kick_params.pitch_decay,
        ParamId::KickAmpDecay => state.kick_params.amp_decay,
        ParamId::KickClick => state.kick_params.click,
        ParamId::KickDrive => state.kick_params.drive,
        ParamId::SnareToneFreq => state.snare_params.tone_freq,
        ParamId::SnareToneDecay => state.snare_params.tone_decay,
        ParamId::SnareNoiseDecay => state.snare_params.noise_decay,
        ParamId::SnareToneMix => state.snare_params.tone_mix,
        ParamId::SnareSnappy => state.snare_params.snappy,
        ParamId::HiHatDecay => state.hihat_params.decay,
        ParamId::HiHatTone => state.hihat_params.tone,
        ParamId::HiHatOpen => state.hihat_params.open,
        ParamId::BassFrequency => state.bass_params.frequency,
        ParamId::BassDecay => state.bass_params.decay,
        ParamId::BassSawMix => state.bass_params.saw_mix,
        ParamId::BassSub => state.bass_params.sub,
    }
}

/// Render the parameter editor view
pub fn render_params(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    editor: &ParamEditorState,
    theme: &Theme,
) {
    // Create outer block
    let block = Block::default()
        .title(Span::styled(
            " Synth Parameters ",
            Style::default().fg(theme.track_label),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Layout: track tabs at top, params below
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Track tabs
            Constraint::Min(4),    // Parameters
        ])
        .split(inner);

    // Render track tabs
    render_track_tabs(frame, chunks[0], editor.track, theme);

    // Render parameters for selected track
    render_param_list(frame, chunks[1], state, editor, theme);
}

/// Render track selection tabs
fn render_track_tabs(frame: &mut Frame, area: Rect, selected: usize, theme: &Theme) {
    let tracks = ["1:KICK", "2:SNARE", "3:HIHAT", "4:BASS"];

    let mut spans = Vec::new();
    for (i, name) in tracks.iter().enumerate() {
        let style = if i == selected {
            Style::default()
                .fg(theme.bg)
                .bg(theme.highlight)
                .bold()
        } else {
            Style::default().fg(theme.dimmed)
        };
        spans.push(Span::styled(format!(" {} ", name), style));
        if i < 3 {
            spans.push(Span::styled(" ", Style::default()));
        }
    }

    let tabs = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.bg))
        .alignment(Alignment::Center);

    frame.render_widget(tabs, area);
}

/// Render the parameter list for the selected track
fn render_param_list(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    editor: &ParamEditorState,
    theme: &Theme,
) {
    let params = ParamId::params_for_track(editor.track);

    let mut lines = Vec::new();

    for (i, param) in params.iter().enumerate() {
        let is_selected = i == editor.param_index;
        let value = get_param_value(state, *param);
        let (min, max, _default) = param.range();

        // Calculate normalized value (0-1)
        let normalized = (value - min) / (max - min);

        // Create value bar
        let bar_width = 20;
        let filled = (normalized * bar_width as f32) as usize;
        let bar: String = (0..bar_width)
            .map(|i| if i < filled { '=' } else { '-' })
            .collect();

        // Format the line
        let name = format!("{:>12}", param.name());
        let value_str = format!("{:>7.1}", value);

        let style = if is_selected {
            Style::default().fg(theme.highlight).bold()
        } else {
            Style::default().fg(theme.fg)
        };

        let bar_style = if is_selected {
            Style::default().fg(theme.grid_active)
        } else {
            Style::default().fg(theme.dimmed)
        };

        let cursor = if is_selected { ">" } else { " " };

        lines.push(Line::from(vec![
            Span::styled(cursor, style),
            Span::styled(name, style),
            Span::styled(" [", Style::default().fg(theme.border)),
            Span::styled(bar, bar_style),
            Span::styled("] ", Style::default().fg(theme.border)),
            Span::styled(value_str, style),
        ]));
    }

    let para = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    frame.render_widget(para, area);
}
