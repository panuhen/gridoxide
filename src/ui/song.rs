use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::audio::SequencerState;
use crate::sequencer::PlaybackMode;
use crate::ui::Theme;

pub struct SongState {
    pub cursor_position: usize,
}

impl SongState {
    pub fn new() -> Self {
        Self {
            cursor_position: 0,
        }
    }
}

impl Default for SongState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the Song/Arrangement view
pub fn render_song(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    song_state: &SongState,
    theme: &Theme,
) {
    let block = Block::default()
        .title(Span::styled(
            " Arrangement ",
            Style::default().fg(theme.track_label),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into left (arrangement) and right (pattern bank)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(inner);

    render_arrangement_list(frame, cols[0], state, song_state, theme);
    render_pattern_bank_grid(frame, cols[1], state, theme);
}

fn render_arrangement_list(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    song_state: &SongState,
    theme: &Theme,
) {
    let mode_str = match state.playback_mode {
        PlaybackMode::Pattern => "PATTERN",
        PlaybackMode::Song => "SONG",
    };

    let mode_style = match state.playback_mode {
        PlaybackMode::Pattern => Style::default().fg(theme.dimmed),
        PlaybackMode::Song => Style::default().fg(theme.meter_high).bold(),
    };

    // Header line
    let header = Line::from(vec![
        Span::styled("ARRANGEMENT ", Style::default().fg(theme.track_label).bold()),
        Span::styled(format!("[{}]", mode_str), mode_style),
    ]);
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, area.y, area.width, 1));

    // Column headers
    let col_header = Line::from(vec![
        Span::styled("  # ", Style::default().fg(theme.dimmed)),
        Span::styled(" Pattern ", Style::default().fg(theme.dimmed)),
        Span::styled(" Repeats", Style::default().fg(theme.dimmed)),
    ]);
    frame.render_widget(
        Paragraph::new(col_header),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );

    if state.arrangement.is_empty() {
        let empty_msg = Span::styled(
            "  (empty - press A to add)",
            Style::default().fg(theme.dimmed),
        );
        frame.render_widget(
            Paragraph::new(Line::from(empty_msg)),
            Rect::new(area.x, area.y + 3, area.width, 1),
        );
        return;
    }

    // Entries
    let max_visible = (area.height as usize).saturating_sub(2);
    let scroll_offset = if song_state.cursor_position >= max_visible {
        song_state.cursor_position - max_visible + 1
    } else {
        0
    };

    for (i, entry) in state.arrangement.entries.iter().enumerate().skip(scroll_offset) {
        let row = i - scroll_offset;
        if row >= max_visible {
            break;
        }

        let y = area.y + 2 + row as u16;
        let is_cursor = i == song_state.cursor_position;
        let is_playing = state.playback_mode == PlaybackMode::Song
            && state.playing
            && i == state.arrangement_position;

        let cursor_marker = if is_cursor { ">" } else { " " };
        let play_marker = if is_playing { " <<" } else { "" };

        let line_style = if is_cursor {
            Style::default().fg(theme.grid_cursor).bold()
        } else if is_playing {
            Style::default().fg(theme.highlight)
        } else {
            Style::default().fg(theme.fg)
        };

        let repeat_bar = "|".repeat(entry.repeats.min(16));
        let line = Line::from(vec![
            Span::styled(format!("{}{:2} ", cursor_marker, i + 1), line_style),
            Span::styled(format!("  [{:02}]  ", entry.pattern), line_style),
            Span::styled(format!("  x{:<2} {}", entry.repeats, repeat_bar), line_style),
            Span::styled(play_marker.to_string(), Style::default().fg(theme.meter_high)),
        ]);

        frame.render_widget(
            Paragraph::new(line),
            Rect::new(area.x, y, area.width, 1),
        );
    }
}

fn render_pattern_bank_grid(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    theme: &Theme,
) {
    // Header
    let header = Span::styled(
        "PATTERN BANK",
        Style::default().fg(theme.track_label).bold(),
    );
    frame.render_widget(
        Paragraph::new(Line::from(header)),
        Rect::new(area.x, area.y, area.width, 1),
    );

    // 4x4 grid of pattern slots
    let cell_width = 6u16;
    for row in 0..4 {
        for col in 0..4 {
            let idx = row * 4 + col;
            let x = area.x + col as u16 * cell_width;
            let y = area.y + 2 + row as u16;

            if y >= area.y + area.height || x + cell_width > area.x + area.width {
                continue;
            }

            let is_current = idx == state.current_pattern;
            let has_content = state.pattern_bank.has_content(idx);

            let style = if is_current {
                Style::default().fg(theme.bg).bg(theme.highlight).bold()
            } else if has_content {
                Style::default().fg(theme.grid_active)
            } else {
                Style::default().fg(theme.dimmed)
            };

            let label = format!("[{:02}]", idx);
            frame.render_widget(
                Paragraph::new(label).style(style),
                Rect::new(x, y, cell_width, 1),
            );
        }
    }

    // Legend below bank grid
    let legend_y = area.y + 7;
    if legend_y < area.y + area.height {
        let legend_lines = vec![
            Line::from(Span::styled(
                ",/. Select pattern",
                Style::default().fg(theme.dimmed),
            )),
            Line::from(Span::styled(
                "A   Add to arrangement",
                Style::default().fg(theme.dimmed),
            )),
            Line::from(Span::styled(
                "D   Delete entry",
                Style::default().fg(theme.dimmed),
            )),
            Line::from(Span::styled(
                "H/L Adjust repeats",
                Style::default().fg(theme.dimmed),
            )),
            Line::from(Span::styled(
                "M   Toggle mode",
                Style::default().fg(theme.dimmed),
            )),
            Line::from(Span::styled(
                "C   Copy pattern",
                Style::default().fg(theme.dimmed),
            )),
            Line::from(Span::styled(
                "X   Clear pattern",
                Style::default().fg(theme.dimmed),
            )),
        ];

        let available = (area.y + area.height - legend_y) as usize;
        for (i, line) in legend_lines.into_iter().take(available).enumerate() {
            frame.render_widget(
                Paragraph::new(line),
                Rect::new(area.x, legend_y + i as u16, area.width, 1),
            );
        }
    }
}
