use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::samples::SampleEntry;
use crate::ui::Theme;

/// State for the sample browser modal
pub struct BrowserState {
    pub entries: Vec<SampleEntry>,
    pub cursor: usize,
    pub scroll: usize,
    pub target_track: usize,
    pub target_track_name: String,
    pub previewing: Option<usize>, // index of previewing entry
}

/// An item in the browser list: either a folder header or a file
enum BrowserItem {
    Folder(String),
    File(usize), // index into entries
}

impl BrowserState {
    pub fn new(entries: Vec<SampleEntry>, target_track: usize, target_track_name: String) -> Self {
        Self {
            entries,
            cursor: 0,
            scroll: 0,
            target_track,
            target_track_name,
            previewing: None,
        }
    }

    fn build_items(&self) -> Vec<BrowserItem> {
        let mut items = Vec::new();
        let mut current_dir = String::new();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.dir != current_dir {
                current_dir = entry.dir.clone();
                items.push(BrowserItem::Folder(current_dir.clone()));
            }
            items.push(BrowserItem::File(i));
        }
        items
    }

    /// Move cursor up, skipping folder headers
    pub fn move_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let items = self.build_items();
        // Find current cursor's position in items
        let current_item_idx = items
            .iter()
            .position(|item| matches!(item, BrowserItem::File(i) if *i == self.cursor));

        if let Some(idx) = current_item_idx {
            // Search backward for next file
            for j in (0..idx).rev() {
                if let BrowserItem::File(entry_idx) = items[j] {
                    self.cursor = entry_idx;
                    return;
                }
            }
        }
    }

    /// Move cursor down, skipping folder headers
    pub fn move_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let items = self.build_items();
        let current_item_idx = items
            .iter()
            .position(|item| matches!(item, BrowserItem::File(i) if *i == self.cursor));

        if let Some(idx) = current_item_idx {
            // Search forward for next file
            for j in (idx + 1)..items.len() {
                if let BrowserItem::File(entry_idx) = items[j] {
                    self.cursor = entry_idx;
                    return;
                }
            }
        }
    }

    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&SampleEntry> {
        self.entries.get(self.cursor)
    }
}

/// Render the sample browser as a modal overlay
pub fn render_browser(
    frame: &mut Frame,
    area: Rect,
    browser: &BrowserState,
    theme: &Theme,
) {
    // Calculate modal area (centered, taking most of the content area)
    let modal_area = centered_rect(80, 90, area);

    // Clear the background
    frame.render_widget(Clear, modal_area);

    let title = format!(
        " Load Sample for track {}: {} ",
        browser.target_track + 1,
        browser.target_track_name,
    );

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme.highlight)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.highlight))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if browser.entries.is_empty() {
        let empty = Paragraph::new("  No .wav files found in sample directories.\n\n  Add .wav files to ~/.gridoxide/samples/")
            .style(Style::default().fg(theme.dimmed).bg(theme.bg));
        frame.render_widget(empty, inner);
        return;
    }

    // Build display items
    let items = browser.build_items();

    // Calculate visible area (leave 2 lines for footer hint)
    let content_height = inner.height.saturating_sub(2) as usize;

    // Find which visual line the cursor is on
    let cursor_visual_line = items
        .iter()
        .enumerate()
        .position(|(_, item)| matches!(item, BrowserItem::File(i) if *i == browser.cursor))
        .unwrap_or(0);

    // Calculate scroll offset to keep cursor visible
    let scroll = if cursor_visual_line < browser.scroll {
        cursor_visual_line
    } else if cursor_visual_line >= browser.scroll + content_height {
        cursor_visual_line - content_height + 1
    } else {
        browser.scroll
    };

    let mut lines = Vec::new();
    for (visual_idx, item) in items.iter().enumerate().skip(scroll).take(content_height) {
        match item {
            BrowserItem::Folder(name) => {
                lines.push(Line::from(Span::styled(
                    format!("  {}/", name),
                    Style::default().fg(theme.track_label).bold(),
                )));
            }
            BrowserItem::File(entry_idx) => {
                let entry = &browser.entries[*entry_idx];
                let is_selected = *entry_idx == browser.cursor;
                let is_previewing = browser.previewing == Some(*entry_idx);

                let cursor_char = if is_selected { ">" } else { " " };
                let preview_marker = if is_previewing { " [playing]" } else { "" };

                let style = if is_selected {
                    Style::default().fg(theme.highlight).bold()
                } else {
                    Style::default().fg(theme.fg)
                };

                let preview_style = Style::default().fg(theme.grid_active);

                let _ = visual_idx; // suppress unused warning

                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", cursor_char), style),
                    Span::styled(entry.name.clone(), style),
                    Span::styled(format!(".wav{}", preview_marker), if is_previewing { preview_style } else { style }),
                ]));
            }
        }
    }

    let para = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    frame.render_widget(para, Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2)));

    // Footer with keybinding hints
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  [Space]", Style::default().fg(theme.grid_active)),
        Span::styled(" Preview  ", Style::default().fg(theme.fg)),
        Span::styled("[Enter]", Style::default().fg(theme.grid_active)),
        Span::styled(" Load  ", Style::default().fg(theme.fg)),
        Span::styled("[Esc]", Style::default().fg(theme.grid_active)),
        Span::styled(" Cancel", Style::default().fg(theme.fg)),
    ]))
    .style(Style::default().bg(theme.bg));

    let footer_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );
    frame.render_widget(footer, footer_area);
}

/// Create a centered rect within a given area
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
