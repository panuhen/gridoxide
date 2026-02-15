use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::audio::SequencerState;
use crate::synth::ParamDescriptor;
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

    /// Move parameter selection up/down
    pub fn move_selection(&mut self, dy: i32, param_count: usize) {
        if param_count == 0 {
            return;
        }
        let len = param_count as i32;
        self.param_index = ((self.param_index as i32 + dy).rem_euclid(len)) as usize;
    }

    /// Switch to a different track
    pub fn switch_track(&mut self, track: usize, num_tracks: usize) {
        if track < num_tracks {
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

/// Get parameter descriptors for a track from state
pub fn get_param_descriptors(state: &SequencerState, track: usize) -> Vec<ParamDescriptor> {
    if track >= state.tracks.len() {
        return vec![];
    }
    // Deserialize from params_snapshot to get descriptors
    // We use the factory to create a temp synth just for descriptors
    // But actually we can get descriptors from the snapshot by using create_synth
    use crate::synth::create_synth;
    let synth = create_synth(state.tracks[track].synth_type, 44100.0, None);
    synth.param_descriptors()
}

/// Get a parameter value from the state's params_snapshot
pub fn get_snapshot_param_value(state: &SequencerState, track: usize, key: &str) -> f32 {
    if track >= state.tracks.len() {
        return 0.0;
    }
    let snapshot = &state.tracks[track].params_snapshot;
    snapshot.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32
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
    render_track_tabs(frame, chunks[0], state, editor.track, theme);

    // Render parameters for selected track
    render_param_list(frame, chunks[1], state, editor, theme);
}

/// Render track selection tabs
fn render_track_tabs(frame: &mut Frame, area: Rect, state: &SequencerState, selected: usize, theme: &Theme) {
    let mut spans = Vec::new();
    for (i, track) in state.tracks.iter().enumerate() {
        let label = format!("{}:{}", i + 1, track.name);
        let style = if i == selected {
            Style::default()
                .fg(theme.bg)
                .bg(theme.highlight)
                .bold()
        } else {
            Style::default().fg(theme.dimmed)
        };
        spans.push(Span::styled(format!(" {} ", label), style));
        if i < state.tracks.len() - 1 {
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
    let descriptors = get_param_descriptors(state, editor.track);

    let mut lines = Vec::new();

    for (i, desc) in descriptors.iter().enumerate() {
        let is_selected = i == editor.param_index;
        let value = get_snapshot_param_value(state, editor.track, &desc.key);

        // Calculate normalized value (0-1)
        let range = desc.max - desc.min;
        let normalized = if range > 0.0 { (value - desc.min) / range } else { 0.0 };

        // Create value bar
        let bar_width = 20;
        let filled = (normalized * bar_width as f32) as usize;
        let bar: String = (0..bar_width)
            .map(|i| if i < filled { '=' } else { '-' })
            .collect();

        // Format the line
        let name = format!("{:>12}", desc.name);
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
