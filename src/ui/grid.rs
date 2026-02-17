use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::sequencer::{Pattern, PlaybackMode, Variation, DEFAULT_TRACKS, STEPS};
use crate::synth::note_name;
use crate::ui::{Theme, dim_color_by_velocity};

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

    pub fn move_cursor(&mut self, dx: i32, dy: i32, num_tracks: usize) {
        let tracks = if num_tracks == 0 { DEFAULT_TRACKS } else { num_tracks };
        self.cursor_step = ((self.cursor_step as i32 + dx).rem_euclid(STEPS as i32)) as usize;
        self.cursor_track = ((self.cursor_track as i32 + dy).rem_euclid(tracks as i32)) as usize;
    }
}

impl Default for GridState {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a note name to fit in cell_width characters
fn format_note(note: u8, cell_width: u16) -> String {
    let name = note_name(note);
    if cell_width >= 3 {
        // Pad or truncate to 3 chars: "C4 " or "C#3"
        if name.len() <= cell_width as usize {
            format!("{:<width$}", name, width = cell_width as usize)
        } else {
            name[..cell_width as usize].to_string()
        }
    } else {
        // 2-char mode: "C4" for naturals, "C#" for sharps (drop octave)
        if name.contains('#') {
            name[..2].to_string()
        } else {
            name[..2.min(name.len())].to_string()
        }
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
    track_names: &[String],
    theme: &Theme,
) {
    let num_tracks = pattern.num_tracks();

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
    let cell_height = if num_tracks > 0 {
        (inner.height / num_tracks as u16).max(1)
    } else {
        1
    };

    // Render each track
    for track in 0..num_tracks {
        let track_y = inner.y + (track as u16 * cell_height);

        if track_y >= inner.y + inner.height {
            break;
        }

        // Track label
        let label = if track < track_names.len() {
            format!("{:>5} ", track_names[track])
        } else {
            format!("{:>5} ", format!("TRK{}", track))
        };
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

            let step_data = pattern.get_step(track, step);
            let is_active = step_data.active;
            let is_cursor = track == grid_state.cursor_track && step == grid_state.cursor_step;
            let is_playhead = playing && step == current_step;

            // Get note display for active steps
            let note_display = if is_active {
                format_note(step_data.note, cell_width)
            } else {
                String::new()
            };

            // Determine cell style
            let display_width = cell_width.min(3);
            let (symbol, style) = if is_cursor {
                if is_active {
                    (
                        format!("{:<width$}", note_display, width = display_width as usize),
                        Style::default()
                            .fg(theme.bg)
                            .bg(theme.grid_cursor)
                            .bold(),
                    )
                } else {
                    (
                        format!("{:<width$}", "[]", width = display_width as usize),
                        Style::default().fg(theme.grid_cursor).bg(theme.bg).bold(),
                    )
                }
            } else if is_playhead {
                if is_active {
                    (
                        format!("{:<width$}", note_display, width = display_width as usize),
                        Style::default()
                            .fg(theme.bg)
                            .bg(theme.highlight)
                            .bold(),
                    )
                } else {
                    (
                        format!("{:<width$}", "::", width = display_width as usize),
                        Style::default().fg(theme.highlight).bg(theme.bg),
                    )
                }
            } else if is_active {
                // Dim color based on velocity
                let velocity_color = dim_color_by_velocity(theme.grid_active, step_data.velocity);
                (
                    format!("{:<width$}", note_display, width = display_width as usize),
                    Style::default().fg(velocity_color).bg(theme.bg),
                )
            } else {
                // Beat markers (every 4 steps)
                if step % 4 == 0 {
                    (
                        format!("{:<width$}", ". ", width = display_width as usize),
                        Style::default().fg(theme.dimmed).bg(theme.bg),
                    )
                } else {
                    (
                        format!("{:<width$}", "- ", width = display_width as usize),
                        Style::default().fg(theme.grid_inactive).bg(theme.bg),
                    )
                }
            };

            frame.render_widget(
                ratatui::widgets::Paragraph::new(symbol).style(style),
                Rect::new(step_x, track_y, display_width, 1),
            );
        }
    }
}

/// Transport info for rendering
pub struct TransportInfo {
    pub playing: bool,
    pub bpm: f32,
    pub current_step: usize,
    pub current_pattern: usize,
    pub playback_mode: PlaybackMode,
    pub arrangement_position: usize,
    pub arrangement_len: usize,
    pub cursor_note: Option<(bool, u8, u8, u8)>, // (active, note, velocity, probability)
    pub pending_pattern: Option<usize>,
    pub current_variation: Variation,
}

/// Render transport status bar
pub fn render_transport(
    frame: &mut Frame,
    area: Rect,
    info: &TransportInfo,
    theme: &Theme,
) {
    let status = if info.playing { "PLAY" } else { "STOP" };
    let status_style = if info.playing {
        Style::default().fg(theme.meter_high).bold()
    } else {
        Style::default().fg(theme.dimmed)
    };

    let mode_str = match info.playback_mode {
        PlaybackMode::Pattern => "PAT",
        PlaybackMode::Song => "SONG",
    };

    let var_str = match info.current_variation {
        Variation::A => "A",
        Variation::B => "B",
    };

    let pat_display = if let Some(pending) = info.pending_pattern {
        format!("Pat: {:02}{}>:{:02}", info.current_pattern, var_str, pending)
    } else {
        format!("Pat: {:02}{}", info.current_pattern, var_str)
    };

    let mut transport_text = vec![
        Span::styled(format!(" {} ", status), status_style),
        Span::styled(" | ", Style::default().fg(theme.border)),
        Span::styled(
            format!("{} ", mode_str),
            Style::default().fg(theme.highlight),
        ),
        Span::styled(" | ", Style::default().fg(theme.border)),
        Span::styled(
            pat_display,
            Style::default().fg(theme.fg),
        ),
        Span::styled(" | ", Style::default().fg(theme.border)),
        Span::styled(
            format!("BPM: {:.0}", info.bpm),
            Style::default().fg(theme.fg),
        ),
        Span::styled(" | ", Style::default().fg(theme.border)),
        Span::styled(
            format!("Step: {:2}/16", info.current_step + 1),
            Style::default().fg(theme.fg),
        ),
    ];

    // Show song position in song mode
    if info.playback_mode == PlaybackMode::Song && info.arrangement_len > 0 {
        transport_text.push(Span::styled(" | ", Style::default().fg(theme.border)));
        transport_text.push(Span::styled(
            format!("Song: {}/{}", info.arrangement_position + 1, info.arrangement_len),
            Style::default().fg(theme.highlight),
        ));
    }

    // Show note/velocity/probability info when cursor is on an active step
    if let Some((active, note, velocity, probability)) = info.cursor_note {
        if active {
            transport_text.push(Span::styled(" | ", Style::default().fg(theme.border)));
            transport_text.push(Span::styled(
                format!("Note: {} Vel: {} Prob: {}%", note_name(note), velocity, probability),
                Style::default().fg(theme.highlight),
            ));
        }
    }

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
