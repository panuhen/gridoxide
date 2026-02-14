use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::audio::SequencerState;
use crate::fx::{FxParamId, MasterFxParamId};
use crate::ui::Theme;

/// State for FX editor view
pub struct FxEditorState {
    /// 0-3 for tracks, 4 for master
    pub track: usize,
    /// Selected parameter row index
    pub param_index: usize,
}

impl FxEditorState {
    pub fn new() -> Self {
        Self {
            track: 0,
            param_index: 0,
        }
    }

    /// Switch to a specific track (0-3) or master (4)
    pub fn select_track(&mut self, track: usize) {
        if track <= 4 {
            self.track = track;
            self.param_index = 0;
        }
    }

    /// Move parameter selection up/down
    pub fn move_selection(&mut self, dy: i32) {
        let count = self.param_count() as i32;
        if count == 0 {
            return;
        }
        self.param_index = ((self.param_index as i32 + dy).rem_euclid(count)) as usize;
    }

    /// Total number of selectable parameter rows for current track
    fn param_count(&self) -> usize {
        if self.track == 4 {
            // Master: 3 reverb params
            3
        } else {
            // Track: filter(3) + dist(2) + delay(3) = 8
            // Row layout: FilterType, Cutoff, Resonance, Drive, DistMix, Time, Feedback, DelayMix
            8
        }
    }

    /// Get the FX section and local param index for the current selection (track mode)
    /// Returns (section: 0=filter, 1=dist, 2=delay, param_within_section)
    pub fn current_section_and_param(&self) -> (usize, usize) {
        match self.param_index {
            0..=2 => (0, self.param_index),     // Filter: type(0), cutoff(1), resonance(2)
            3..=4 => (1, self.param_index - 3), // Dist: drive(0), mix(1)
            5..=7 => (2, self.param_index - 5), // Delay: time(0), feedback(1), mix(2)
            _ => (0, 0),
        }
    }
}

impl Default for FxEditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a track FX parameter value from state
pub fn get_fx_param_value(state: &SequencerState, track: usize, param: FxParamId) -> f32 {
    if track >= 4 {
        return 0.0;
    }
    let fx = &state.track_fx[track];
    match param {
        FxParamId::FilterCutoff => fx.filter_cutoff,
        FxParamId::FilterResonance => fx.filter_resonance,
        FxParamId::DistDrive => fx.dist_drive,
        FxParamId::DistMix => fx.dist_mix,
        FxParamId::DelayTime => fx.delay_time,
        FxParamId::DelayFeedback => fx.delay_feedback,
        FxParamId::DelayMix => fx.delay_mix,
    }
}

/// Get a master FX parameter value from state
pub fn get_master_fx_param_value(state: &SequencerState, param: MasterFxParamId) -> f32 {
    match param {
        MasterFxParamId::ReverbDecay => state.master_fx.reverb_decay,
        MasterFxParamId::ReverbMix => state.master_fx.reverb_mix,
        MasterFxParamId::ReverbDamping => state.master_fx.reverb_damping,
    }
}

/// Render the FX editor view
pub fn render_fx(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    editor: &FxEditorState,
    theme: &Theme,
) {
    let block = Block::default()
        .title(Span::styled(
            " Effects ",
            Style::default().fg(theme.track_label),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Track tabs
            Constraint::Min(4),    // FX parameters
        ])
        .split(inner);

    render_fx_tabs(frame, chunks[0], editor.track, theme);

    if editor.track == 4 {
        render_master_fx_params(frame, chunks[1], state, editor, theme);
    } else {
        render_track_fx_params(frame, chunks[1], state, editor, theme);
    }
}

/// Render track/master tabs for FX view
fn render_fx_tabs(frame: &mut Frame, area: Rect, selected: usize, theme: &Theme) {
    let tabs = ["1:KICK", "2:SNARE", "3:HIHAT", "4:BASS", "5:MASTER"];

    let mut spans = Vec::new();
    for (i, name) in tabs.iter().enumerate() {
        let style = if i == selected {
            Style::default()
                .fg(theme.bg)
                .bg(theme.highlight)
                .bold()
        } else {
            Style::default().fg(theme.dimmed)
        };
        spans.push(Span::styled(format!(" {} ", name), style));
        if i < tabs.len() - 1 {
            spans.push(Span::styled(" ", Style::default()));
        }
    }

    let tabs_widget = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.bg))
        .alignment(Alignment::Center);

    frame.render_widget(tabs_widget, area);
}

/// Render per-track FX parameters (filter + distortion + delay)
fn render_track_fx_params(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    editor: &FxEditorState,
    theme: &Theme,
) {
    let track = editor.track;
    if track >= 4 {
        return;
    }
    let fx = &state.track_fx[track];

    let mut lines = Vec::new();
    let mut row_idx = 0usize;

    // --- FILTER ---
    let filter_status = if fx.filter_enabled { " ON" } else { "OFF" };
    let filter_status_style = if fx.filter_enabled {
        Style::default().fg(theme.meter_low).bold()
    } else {
        Style::default().fg(theme.dimmed)
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  FILTER",
            Style::default().fg(theme.track_label).bold(),
        ),
        Span::raw("                                        "),
        Span::styled(format!("[{}]", filter_status), filter_status_style),
    ]));

    // Filter Type
    lines.push(render_param_row(
        row_idx == editor.param_index,
        "Type",
        fx.filter_type.name(),
        0.0, // not used for type display
        true, // is_type
        theme,
    ));
    row_idx += 1;

    // Filter Cutoff
    let cutoff_norm = (fx.filter_cutoff - 20.0) / (20000.0 - 20.0);
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Cutoff",
        cutoff_norm,
        &format!("{:.0} Hz", fx.filter_cutoff),
        theme,
    ));
    row_idx += 1;

    // Filter Resonance
    let res_norm = fx.filter_resonance / 0.95;
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Resonance",
        res_norm,
        &format!("{:.2}", fx.filter_resonance),
        theme,
    ));
    row_idx += 1;

    lines.push(Line::from("")); // spacer

    // --- DISTORTION ---
    let dist_status = if fx.dist_enabled { " ON" } else { "OFF" };
    let dist_status_style = if fx.dist_enabled {
        Style::default().fg(theme.meter_low).bold()
    } else {
        Style::default().fg(theme.dimmed)
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  DISTORTION",
            Style::default().fg(theme.track_label).bold(),
        ),
        Span::raw("                                    "),
        Span::styled(format!("[{}]", dist_status), dist_status_style),
    ]));

    // Drive
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Drive",
        fx.dist_drive,
        &format!("{:.2}", fx.dist_drive),
        theme,
    ));
    row_idx += 1;

    // Dist Mix
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Mix",
        fx.dist_mix,
        &format!("{:.2}", fx.dist_mix),
        theme,
    ));
    row_idx += 1;

    lines.push(Line::from("")); // spacer

    // --- DELAY ---
    let delay_status = if fx.delay_enabled { " ON" } else { "OFF" };
    let delay_status_style = if fx.delay_enabled {
        Style::default().fg(theme.meter_low).bold()
    } else {
        Style::default().fg(theme.dimmed)
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  DELAY",
            Style::default().fg(theme.track_label).bold(),
        ),
        Span::raw("                                         "),
        Span::styled(format!("[{}]", delay_status), delay_status_style),
    ]));

    // Time
    let time_norm = (fx.delay_time - 10.0) / (500.0 - 10.0);
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Time",
        time_norm,
        &format!("{:.0} ms", fx.delay_time),
        theme,
    ));
    row_idx += 1;

    // Feedback
    let fb_norm = fx.delay_feedback / 0.9;
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Feedback",
        fb_norm,
        &format!("{:.2}", fx.delay_feedback),
        theme,
    ));
    row_idx += 1;

    // Delay Mix
    lines.push(render_value_row(
        row_idx == editor.param_index,
        "Mix",
        fx.delay_mix,
        &format!("{:.2}", fx.delay_mix),
        theme,
    ));
    let _ = row_idx;

    let para = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    frame.render_widget(para, area);
}

/// Render master FX parameters (reverb)
fn render_master_fx_params(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    editor: &FxEditorState,
    theme: &Theme,
) {
    let mfx = &state.master_fx;

    let mut lines = Vec::new();

    let reverb_status = if mfx.reverb_enabled { " ON" } else { "OFF" };
    let reverb_status_style = if mfx.reverb_enabled {
        Style::default().fg(theme.meter_low).bold()
    } else {
        Style::default().fg(theme.dimmed)
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  REVERB",
            Style::default().fg(theme.track_label).bold(),
        ),
        Span::raw("                                        "),
        Span::styled(format!("[{}]", reverb_status), reverb_status_style),
    ]));

    // Decay
    let decay_norm = (mfx.reverb_decay - 0.1) / (0.95 - 0.1);
    lines.push(render_value_row(
        0 == editor.param_index,
        "Decay",
        decay_norm,
        &format!("{:.2}", mfx.reverb_decay),
        theme,
    ));

    // Mix
    lines.push(render_value_row(
        1 == editor.param_index,
        "Mix",
        mfx.reverb_mix,
        &format!("{:.2}", mfx.reverb_mix),
        theme,
    ));

    // Damping
    lines.push(render_value_row(
        2 == editor.param_index,
        "Damping",
        mfx.reverb_damping,
        &format!("{:.2}", mfx.reverb_damping),
        theme,
    ));

    let para = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    frame.render_widget(para, area);
}

/// Render a parameter row with bar visualization
fn render_value_row<'a>(
    is_selected: bool,
    name: &str,
    normalized: f32,
    value_str: &str,
    theme: &Theme,
) -> Line<'a> {
    let bar_width = 16;
    let filled = (normalized.clamp(0.0, 1.0) * bar_width as f32) as usize;
    let bar: String = (0..bar_width)
        .map(|i| if i < filled { '=' } else { '-' })
        .collect();

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

    let cursor = if is_selected { "> " } else { "  " };

    Line::from(vec![
        Span::styled(cursor.to_string(), style),
        Span::styled(format!("{:>12}", name), style),
        Span::styled("  [", Style::default().fg(theme.border)),
        Span::styled(bar, bar_style),
        Span::styled("] ", Style::default().fg(theme.border)),
        Span::styled(value_str.to_string(), style),
    ])
}

/// Render a type/enum parameter row
fn render_param_row<'a>(
    is_selected: bool,
    name: &str,
    value_str: &str,
    _normalized: f32,
    _is_type: bool,
    theme: &Theme,
) -> Line<'a> {
    let style = if is_selected {
        Style::default().fg(theme.highlight).bold()
    } else {
        Style::default().fg(theme.fg)
    };

    let cursor = if is_selected { "> " } else { "  " };

    Line::from(vec![
        Span::styled(cursor.to_string(), style),
        Span::styled(format!("{:>12}", name), style),
        Span::styled("   ", Style::default()),
        Span::styled(format!("  {}", value_str), style),
    ])
}
