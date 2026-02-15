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

    pub fn select_track(&mut self, track: usize, num_tracks: usize) {
        if track < num_tracks {
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
    let num_tracks = state.tracks.len();

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

    if num_tracks == 0 {
        return;
    }

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

    // Calculate column width for each track
    let col_width = (inner.width / num_tracks as u16).max(8);

    // Track headers
    render_track_headers(frame, chunks[0], state, mixer_state, col_width, theme);

    // Volume faders (vertical bars)
    render_volume_faders(frame, chunks[1], state, mixer_state, col_width, theme);

    // Volume values
    render_value_row(
        frame,
        chunks[2],
        state,
        mixer_state,
        MixerField::Volume,
        col_width,
        theme,
        |t| format!("{:.2}", t.volume),
        "VOL",
    );

    // Pan values
    render_value_row(
        frame,
        chunks[3],
        state,
        mixer_state,
        MixerField::Pan,
        col_width,
        theme,
        |t| {
            if t.pan < -0.05 {
                format!("L{:.1}", -t.pan)
            } else if t.pan > 0.05 {
                format!("R{:.1}", t.pan)
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
        state,
        mixer_state,
        MixerField::Mute,
        col_width,
        theme,
        |t| t.mute,
        "M",
        "MUTE",
    );

    // Solo toggles
    render_toggle_row(
        frame,
        chunks[5],
        state,
        mixer_state,
        MixerField::Solo,
        col_width,
        theme,
        |t| t.solo,
        "S",
        "SOLO",
    );
}

fn render_track_headers(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    mixer_state: &MixerState,
    col_width: u16,
    theme: &Theme,
) {
    let num_tracks = state.tracks.len();
    for i in 0..num_tracks {
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

        let label = format!("{:^width$}", state.tracks[i].name, width = col_width as usize);
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

    let num_tracks = state.tracks.len();
    let any_solo = state.tracks.iter().any(|t| t.solo);

    for track in 0..num_tracks {
        let x = area.x + track as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }

        let volume = state.tracks[track].volume;
        let filled = (volume * fader_height as f32).round() as u16;
        let is_selected =
            track == mixer_state.selected_track && mixer_state.selected_field == MixerField::Volume;
        let is_muted = state.tracks[track].mute;
        let is_audible = if any_solo {
            state.tracks[track].solo
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

            let block_char = if is_filled { "\u{2588}" } else { "\u{2591}" };
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
    state: &SequencerState,
    mixer_state: &MixerState,
    field: MixerField,
    col_width: u16,
    theme: &Theme,
    format_fn: F,
    label: &str,
) where
    F: Fn(&crate::audio::TrackState) -> String,
{
    let num_tracks = state.tracks.len();
    for track in 0..num_tracks {
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

        let text = format_fn(&state.tracks[track]);
        let display = format!("{:^width$}", text, width = col_width as usize);
        frame.render_widget(
            Paragraph::new(display).style(style),
            Rect::new(x, area.y, col_width, 1),
        );
    }

    // Row label on the right if space allows
    let label_x = area.x + num_tracks as u16 * col_width;
    if label_x + label.len() as u16 <= area.x + area.width {
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(Style::default().fg(theme.dimmed)),
            Rect::new(label_x, area.y, (area.width - num_tracks as u16 * col_width).min(6), 1),
        );
    }
}

fn render_toggle_row<F>(
    frame: &mut Frame,
    area: Rect,
    state: &SequencerState,
    mixer_state: &MixerState,
    field: MixerField,
    col_width: u16,
    theme: &Theme,
    get_value: F,
    active_char: &str,
    label: &str,
) where
    F: Fn(&crate::audio::TrackState) -> bool,
{
    let num_tracks = state.tracks.len();
    for track in 0..num_tracks {
        let x = area.x + track as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }

        let is_selected =
            track == mixer_state.selected_track && mixer_state.selected_field == field;
        let is_active = get_value(&state.tracks[track]);

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
    let label_x = area.x + num_tracks as u16 * col_width;
    if label_x + label.len() as u16 <= area.x + area.width {
        frame.render_widget(
            Paragraph::new(format!(" {}", label)).style(Style::default().fg(theme.dimmed)),
            Rect::new(label_x, area.y, (area.width - num_tracks as u16 * col_width).min(6), 1),
        );
    }
}
