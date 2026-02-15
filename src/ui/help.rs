use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::ui::Theme;

pub struct HelpState {
    pub scroll: usize,
}

impl HelpState {
    pub fn new() -> Self {
        Self { scroll: 0 }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, max_lines: usize, visible: usize) {
        if max_lines > visible && self.scroll < max_lines - visible {
            self.scroll += 1;
        }
    }
}

impl Default for HelpState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the Help view showing all keybindings
pub fn render_help(
    frame: &mut Frame,
    area: Rect,
    help_state: &HelpState,
    theme: &Theme,
) {
    let block = Block::default()
        .title(Span::styled(
            " Help ",
            Style::default().fg(theme.track_label),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = build_help_lines(theme);
    let total_lines = lines.len();
    let visible = inner.height as usize;

    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(help_state.scroll)
        .take(visible)
        .collect();

    let para = Paragraph::new(visible_lines).style(Style::default().bg(theme.bg));
    frame.render_widget(para, inner);

    // Scroll indicator
    if total_lines > visible {
        let pct = if total_lines <= visible {
            100
        } else {
            (help_state.scroll * 100) / (total_lines - visible)
        };
        let indicator = format!(" {}% ", pct);
        let indicator_widget = Paragraph::new(indicator)
            .style(Style::default().fg(theme.dimmed));
        let indicator_area = Rect::new(
            inner.x + inner.width.saturating_sub(6),
            inner.y + inner.height.saturating_sub(1),
            6,
            1,
        );
        frame.render_widget(indicator_widget, indicator_area);
    }
}

/// Total number of help lines (for scroll bounds)
pub fn help_line_count(theme: &Theme) -> usize {
    build_help_lines(theme).len()
}

fn build_help_lines(theme: &Theme) -> Vec<Line<'static>> {
    let header_style = Style::default().fg(theme.highlight).bold();
    let key_style = Style::default().fg(theme.grid_active);
    let desc_style = Style::default().fg(theme.fg);
    let dim_style = Style::default().fg(theme.dimmed);

    let mut lines = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        "  GRIDOXIDE KEYBINDINGS",
        header_style,
    )));
    lines.push(Line::from(""));

    // Global
    lines.push(Line::from(Span::styled("  GLOBAL", header_style)));
    lines.push(Line::from(Span::styled(
        "  ──────────────────────────────────────",
        dim_style,
    )));
    add_key(&mut lines, "  Tab       ", "Cycle views: Grid > Params > Mixer > FX > Song", key_style, desc_style);
    add_key(&mut lines, "  Esc       ", "Return to Grid view", key_style, desc_style);
    add_key(&mut lines, "  G         ", "Toggle Help view", key_style, desc_style);
    add_key(&mut lines, "  Q         ", "Quit", key_style, desc_style);
    add_key(&mut lines, "  P         ", "Play / Pause toggle", key_style, desc_style);
    add_key(&mut lines, "  S         ", "Stop (reset to step 0)", key_style, desc_style);
    add_key(&mut lines, "  Ctrl+S    ", "Save project (.grox)", key_style, desc_style);
    add_key(&mut lines, "  Ctrl+O    ", "Load project (.grox)", key_style, desc_style);
    add_key(&mut lines, "  Ctrl+E    ", "Export current pattern as WAV", key_style, desc_style);
    add_key(&mut lines, "  Ctrl+W    ", "Export song arrangement as WAV", key_style, desc_style);
    lines.push(Line::from(""));

    // Grid
    lines.push(Line::from(Span::styled("  GRID VIEW", header_style)));
    lines.push(Line::from(Span::styled(
        "  ──────────────────────────────────────",
        dim_style,
    )));
    add_key(&mut lines, "  Arrows    ", "Move cursor (also H/J/K/L)", key_style, desc_style);
    add_key(&mut lines, "  Space     ", "Toggle step on/off", key_style, desc_style);
    add_key(&mut lines, "  Enter     ", "Toggle step on/off", key_style, desc_style);
    add_key(&mut lines, "  [ / ]     ", "Note down/up 1 semitone", key_style, desc_style);
    add_key(&mut lines, "  { / }     ", "Note down/up 1 octave", key_style, desc_style);
    add_key(&mut lines, "  + / -     ", "BPM up/down by 5", key_style, desc_style);
    add_key(&mut lines, "  C         ", "Clear current track", key_style, desc_style);
    add_key(&mut lines, "  F         ", "Fill current track", key_style, desc_style);
    add_key(&mut lines, "  , / .     ", "Previous / next pattern", key_style, desc_style);
    lines.push(Line::from(""));

    // Params
    lines.push(Line::from(Span::styled("  PARAMS VIEW", header_style)));
    lines.push(Line::from(Span::styled(
        "  ──────────────────────────────────────",
        dim_style,
    )));
    add_key(&mut lines, "  1-9       ", "Select track", key_style, desc_style);
    add_key(&mut lines, "  Up/Down   ", "Select parameter", key_style, desc_style);
    add_key(&mut lines, "  Left/Right", "Adjust value (fine)", key_style, desc_style);
    add_key(&mut lines, "  [ / ]     ", "Adjust value (coarse)", key_style, desc_style);
    lines.push(Line::from(""));

    // Mixer
    lines.push(Line::from(Span::styled("  MIXER VIEW", header_style)));
    lines.push(Line::from(Span::styled(
        "  ──────────────────────────────────────",
        dim_style,
    )));
    add_key(&mut lines, "  1-9       ", "Select track", key_style, desc_style);
    add_key(&mut lines, "  Up/Down   ", "Select field (Vol/Pan/Mute/Solo)", key_style, desc_style);
    add_key(&mut lines, "  Left/Right", "Adjust value or toggle", key_style, desc_style);
    add_key(&mut lines, "  M         ", "Toggle mute", key_style, desc_style);
    add_key(&mut lines, "  O         ", "Toggle solo", key_style, desc_style);
    lines.push(Line::from(""));

    // FX
    lines.push(Line::from(Span::styled("  FX VIEW", header_style)));
    lines.push(Line::from(Span::styled(
        "  ──────────────────────────────────────",
        dim_style,
    )));
    add_key(&mut lines, "  1-9       ", "Select track", key_style, desc_style);
    add_key(&mut lines, "  M         ", "Select master bus", key_style, desc_style);
    add_key(&mut lines, "  Up/Down   ", "Select parameter", key_style, desc_style);
    add_key(&mut lines, "  Left/Right", "Adjust value (fine)", key_style, desc_style);
    add_key(&mut lines, "  [ / ]     ", "Adjust value (coarse)", key_style, desc_style);
    add_key(&mut lines, "  Space     ", "Toggle effect on/off", key_style, desc_style);
    lines.push(Line::from(""));

    // Song
    lines.push(Line::from(Span::styled("  SONG VIEW", header_style)));
    lines.push(Line::from(Span::styled(
        "  ──────────────────────────────────────",
        dim_style,
    )));
    add_key(&mut lines, "  Up/Down   ", "Navigate arrangement entries", key_style, desc_style);
    add_key(&mut lines, "  Left/Right", "Adjust repeat count", key_style, desc_style);
    add_key(&mut lines, "  + / -     ", "Cycle pattern index on entry", key_style, desc_style);
    add_key(&mut lines, "  A         ", "Append current pattern to arrangement", key_style, desc_style);
    add_key(&mut lines, "  D / Del   ", "Delete entry at cursor", key_style, desc_style);
    add_key(&mut lines, "  Enter     ", "Set entry to current pattern", key_style, desc_style);
    add_key(&mut lines, "  M         ", "Toggle Pattern/Song mode", key_style, desc_style);
    add_key(&mut lines, "  , / .     ", "Previous / next pattern", key_style, desc_style);
    add_key(&mut lines, "  C         ", "Copy pattern to empty slot", key_style, desc_style);
    add_key(&mut lines, "  X         ", "Clear current pattern", key_style, desc_style);

    lines
}

fn add_key(lines: &mut Vec<Line<'static>>, key: &str, desc: &str, key_style: Style, desc_style: Style) {
    lines.push(Line::from(vec![
        Span::styled(key.to_string(), key_style),
        Span::styled(format!("  {}", desc), desc_style),
    ]));
}
