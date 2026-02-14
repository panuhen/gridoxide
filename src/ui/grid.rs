use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::sequencer::{Pattern, TrackType, STEPS, TRACKS};
use crate::ui::Theme;

/// Grid cursor and playhead state
pub struct GridState {
    pub cursor_track: usize,
    pub cursor_step: usize,
}

impl GridState {
    pub fn new() -> Self {
        Self {
            cursor_track: 0,
            cursor_step: 0,
        }
    }

    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        self.cursor_step = ((self.cursor_step as i32 + dx).rem_euclid(STEPS as i32)) as usize;
        self.cursor_track = ((self.cursor_track as i32 + dy).rem_euclid(TRACKS as i32)) as usize;
    }
}

impl Default for GridState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the step sequencer grid
pub fn render_grid(
    frame: &mut Frame,
    area: Rect,
    pattern: &Pattern,
    grid_state: &GridState,
    current_step: usize,
    playing: bool,
    theme: &Theme,
) {
    // Create outer block
    let block = Block::default()
        .title(Span::styled(
            " Pattern ",
            Style::default().fg(theme.track_label),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate cell dimensions
    // Track label width + 16 steps
    let label_width = 6u16;
    let available_width = inner.width.saturating_sub(label_width);
    let cell_width = (available_width / STEPS as u16).max(2);
    let cell_height = (inner.height / TRACKS as u16).max(1);

    // Render each track
    for track in 0..TRACKS {
        let track_y = inner.y + (track as u16 * cell_height);

        if track_y >= inner.y + inner.height {
            break;
        }

        // Track label
        let track_type = TrackType::from_index(track).unwrap();
        let label = format!("{:>5} ", track_type.name());
        let label_style = if track == grid_state.cursor_track {
            Style::default().fg(theme.highlight).bold()
        } else {
            Style::default().fg(theme.track_label)
        };

        frame.render_widget(
            ratatui::widgets::Paragraph::new(label).style(label_style),
            Rect::new(inner.x, track_y, label_width, 1),
        );

        // Steps
        for step in 0..STEPS {
            let step_x = inner.x + label_width + (step as u16 * cell_width);

            if step_x >= inner.x + inner.width {
                break;
            }

            let is_active = pattern.get(track, step);
            let is_cursor = track == grid_state.cursor_track && step == grid_state.cursor_step;
            let is_playhead = playing && step == current_step;

            // Determine cell style
            let (symbol, style) = if is_cursor {
                if is_active {
                    (
                        "[]",
                        Style::default()
                            .fg(theme.bg)
                            .bg(theme.grid_cursor)
                            .bold(),
                    )
                } else {
                    (
                        "[]",
                        Style::default().fg(theme.grid_cursor).bg(theme.bg).bold(),
                    )
                }
            } else if is_playhead {
                if is_active {
                    (
                        "##",
                        Style::default()
                            .fg(theme.bg)
                            .bg(theme.highlight)
                            .bold(),
                    )
                } else {
                    (
                        "::",
                        Style::default().fg(theme.highlight).bg(theme.bg),
                    )
                }
            } else if is_active {
                (
                    "##",
                    Style::default().fg(theme.grid_active).bg(theme.bg),
                )
            } else {
                // Beat markers (every 4 steps)
                if step % 4 == 0 {
                    (
                        ". ",
                        Style::default().fg(theme.dimmed).bg(theme.bg),
                    )
                } else {
                    (
                        "- ",
                        Style::default().fg(theme.grid_inactive).bg(theme.bg),
                    )
                }
            };

            frame.render_widget(
                ratatui::widgets::Paragraph::new(symbol).style(style),
                Rect::new(step_x, track_y, cell_width.min(2), 1),
            );
        }
    }
}

/// Render transport status bar
pub fn render_transport(
    frame: &mut Frame,
    area: Rect,
    playing: bool,
    bpm: f32,
    current_step: usize,
    theme: &Theme,
) {
    let status = if playing { "PLAY" } else { "STOP" };
    let status_style = if playing {
        Style::default().fg(theme.meter_high).bold()
    } else {
        Style::default().fg(theme.dimmed)
    };

    let transport_text = vec![
        Span::styled(format!(" {} ", status), status_style),
        Span::styled(" | ", Style::default().fg(theme.border)),
        Span::styled(
            format!("BPM: {:.0}", bpm),
            Style::default().fg(theme.fg),
        ),
        Span::styled(" | ", Style::default().fg(theme.border)),
        Span::styled(
            format!("Step: {:2}/16", current_step + 1),
            Style::default().fg(theme.fg),
        ),
    ];

    let transport = ratatui::widgets::Paragraph::new(Line::from(transport_text))
        .style(Style::default().bg(theme.bg))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.bg)),
        );

    frame.render_widget(transport, area);
}
