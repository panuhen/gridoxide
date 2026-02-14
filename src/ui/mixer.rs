use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::audio::SequencerState;
use crate::ui::Theme;

/// Which field is selected in the mixer
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MixerField {
    Volume,
    Pan,
    Mute,
    Solo,
}

impl MixerField {
    pub fn count() -> usize {
        4
    }

    pub fn from_index(i: usize) -> Self {
        match i % 4 {
            0 => MixerField::Volume,
            1 => MixerField::Pan,
            2 => MixerField::Mute,
            3 => MixerField::Solo,
            _ => unreachable!(),
        }
    }

    pub fn index(self) -> usize {
        match self {
            MixerField::Volume => 0,
            MixerField::Pan => 1,
            MixerField::Mute => 2,
            MixerField::Solo => 3,
        }
    }
}

/// State for mixer view
pub struct MixerState {
    pub selected_track: usize,
    pub selected_field: MixerField,
}

impl MixerState {
    pub fn new() -> Self {
        Self {
            selected_track: 0,
            selected_field: MixerField::Volume,
        }
    }

    pub fn select_track(&mut self, track: usize) {
        if track < 4 {
            self.selected_track = track;
        }
    }

    pub fn move_field(&mut self, dy: i32) {
        let count = MixerField::count() as i32;
        let idx = (self.selected_field.index() as i32 + dy).rem_euclid(count);
        self.selected_field = MixerField::from_index(idx as usize);
    }
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the mixer view with channel strips
pub fn render_mixer(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    mixer_state: &MixerState,
    theme: &Theme,
) {
    let block = Block::default()
        .title(Span::styled(
            " Mixer ",
            Style::default().fg(theme.track_label),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Layout: track headers, faders, values
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Track name headers
            Constraint::Min(4),   // Volume faders
            Constraint::Length(1), // Volume values
            Constraint::Length(1), // Pan values
            Constraint::Length(1), // Mute toggles
            Constraint::Length(1), // Solo toggles
        ])
        .split(inner);

    let track_names = ["KICK", "SNARE", "HIHAT", "BASS"];

    // Calculate column width for each track
    let col_width = (inner.width / 4).max(8);

    // Track headers
    render_track_headers(frame, chunks[0], track_names, mixer_state, col_width, theme);

    // Volume faders (vertical bars)
    render_volume_faders(frame, chunks[1], state, mixer_state, col_width, theme);

    // Volume values
    render_value_row(
        frame,
        chunks[2],
        &state.track_volumes,
        mixer_state,
        MixerField::Volume,
        col_width,
        theme,
        |v| format!("{:.2}", v),
        "VOL",
    );

    // Pan values
    render_value_row(
        frame,
        chunks[3],
        &state.track_pans,
        mixer_state,
        MixerField::Pan,
        col_width,
        theme,
        |v| {
            if *v < -0.05 {
                format!("L{:.1}", -v)
            } else if *v > 0.05 {
                format!("R{:.1}", v)
            } else {
                "C".to_string()
            }
        },
        "PAN",
    );

    // Mute toggles
    render_toggle_row(
        frame,
        chunks[4],
        &state.track_mutes,
        mixer_state,
        MixerField::Mute,
        col_width,
        theme,
        "M",
        "MUTE",
    );

    // Solo toggles
    render_toggle_row(
        frame,
        chunks[5],
        &state.track_solos,
        mixer_state,
        MixerField::Solo,
        col_width,
        theme,
        "S",
        "SOLO",
    );
}

fn render_track_headers(
    frame: &mut Frame,
    area: Rect,
    names: [&str; 4],
    mixer_state: &MixerState,
    col_width: u16,
    theme: &Theme,
) {
    for (i, name) in names.iter().enumerate() {
        let x = area.x + i as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }

        let style = if i == mixer_state.selected_track {
            Style::default()
                .fg(theme.bg)
                .bg(theme.highlight)
                .bold()
        } else {
            Style::default().fg(theme.track_label)
        };

        let label = format!("{:^width$}", name, width = col_width as usize);
        frame.render_widget(
            Paragraph::new(label).style(style),
            Rect::new(x, area.y, col_width, 1),
        );
    }
}

fn render_volume_faders(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    mixer_state: &MixerState,
    col_width: u16,
    theme: &Theme,
) {
    let fader_height = area.height;
    if fader_height == 0 {
        return;
    }

    for track in 0..4 {
        let x = area.x + track as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }

        let volume = state.track_volumes[track];
        let filled = (volume * fader_height as f32).round() as u16;
        let is_selected =
            track == mixer_state.selected_track && mixer_state.selected_field == MixerField::Volume;
        let is_muted = state.track_mutes[track];
        let any_solo = state.track_solos.iter().any(|&s| s);
        let is_audible = if any_solo {
            state.track_solos[track]
        } else {
            !is_muted
        };

        // Center the fader bar in the column
        let bar_width = (col_width - 2).max(2).min(4);
        let bar_x = x + (col_width - bar_width) / 2;

        for row in 0..fader_height {
            let y = area.y + (fader_height - 1 - row);
            let is_filled = row < filled;

            let style = if !is_audible {
                Style::default().fg(theme.dimmed).bg(theme.bg)
            } else if is_selected {
                if is_filled {
                    Style::default().fg(theme.highlight).bg(theme.bg)
                } else {
                    Style::default().fg(theme.border).bg(theme.bg)
                }
            } else if is_filled {
                // Color based on level
                let level = row as f32 / fader_height as f32;
                let color = if level > 0.85 {
                    theme.meter_high
                } else if level > 0.6 {
                    theme.meter_mid
                } else {
                    theme.meter_low
                };
                Style::default().fg(color).bg(theme.bg)
            } else {
                Style::default().fg(theme.grid_inactive).bg(theme.bg)
            };

            let block_char = if is_filled { "█" } else { "░" };
            let bar: String = block_char.repeat(bar_width as usize);
            frame.render_widget(
                Paragraph::new(bar).style(style),
                Rect::new(bar_x, y, bar_width, 1),
            );
        }
    }
}

fn render_value_row<F>(
    frame: &mut Frame,
    area: Rect,
    values: &[f32; 4],
    mixer_state: &MixerState,
    field: MixerField,
    col_width: u16,
    theme: &Theme,
    format_fn: F,
    label: &str,
) where
    F: Fn(&f32) -> String,
{
    for track in 0..4 {
        let x = area.x + track as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }

        let is_selected =
            track == mixer_state.selected_track && mixer_state.selected_field == field;

        let style = if is_selected {
            Style::default().fg(theme.highlight).bold()
        } else {
            Style::default().fg(theme.fg)
        };

        let text = format_fn(&values[track]);
        let display = format!("{:^width$}", text, width = col_width as usize);
        frame.render_widget(
            Paragraph::new(display).style(style),
            Rect::new(x, area.y, col_width, 1),
        );
    }

    // Row label on the right if space allows
    let label_x = area.x + 4 * col_width;
    if label_x + label.len() as u16 <= area.x + area.width {
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(Style::default().fg(theme.dimmed)),
            Rect::new(label_x, area.y, (area.width - 4 * col_width).min(6), 1),
        );
    }
}

fn render_toggle_row(
    frame: &mut Frame,
    area: Rect,
    values: &[bool; 4],
    mixer_state: &MixerState,
    field: MixerField,
    col_width: u16,
    theme: &Theme,
    active_char: &str,
    label: &str,
) {
    for track in 0..4 {
        let x = area.x + track as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }

        let is_selected =
            track == mixer_state.selected_track && mixer_state.selected_field == field;
        let is_active = values[track];

        let text = if is_active {
            format!("[{}]", active_char)
        } else {
            "[ ]".to_string()
        };

        let style = if is_selected {
            if is_active {
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.highlight)
                    .bold()
            } else {
                Style::default().fg(theme.highlight).bold()
            }
        } else if is_active {
            Style::default().fg(theme.meter_high).bold()
        } else {
            Style::default().fg(theme.dimmed)
        };

        let display = format!("{:^width$}", text, width = col_width as usize);
        frame.render_widget(
            Paragraph::new(display).style(style),
            Rect::new(x, area.y, col_width, 1),
        );
    }

    // Row label
    let label_x = area.x + 4 * col_width;
    if label_x + label.len() as u16 <= area.x + area.width {
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(Style::default().fg(theme.dimmed)),
            Rect::new(label_x, area.y, (area.width - 4 * col_width).min(6), 1),
        );
    }
}
