//! The main journal view: every entry's current version, newest at the
//! bottom, in creation order. See FEATURES.md if you want to change how
//! entries are listed (e.g. add filtering or search).

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Background marking the selected entry.
const SELECTED_BG: Color = Color::Rgb(38, 40, 54);

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(" TextPoppup — {} entries ", app.store.entries.len());
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;

    let mut lines: Vec<Line> = Vec::new();
    // Row range the selected entry occupies, so the view can scroll to it.
    let mut sel_first = 0usize;
    let mut sel_last = 0usize;

    if app.store.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "No entries yet — press 'n' to write one.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    for (i, entry) in app.store.entries.iter().enumerate() {
        let selected = i == app.selected;
        let version = entry.current();
        if selected {
            sel_first = lines.len();
        }

        let mut header = vec![
            Span::styled(
                format!("[ID: {}] ", entry.id),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                entry.created_at.format("%d/%m/%Y %H:%M:%S").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ];
        if let Some(title) = entry.title() {
            header.push(Span::styled(
                format!("  {title}"),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if entry.edit_count() > 0 {
            header.push(Span::styled(
                format!("  (edited {}x)", entry.edit_count()),
                Style::default().fg(Color::Yellow),
            ));
        }
        let header_style = if selected {
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(SELECTED_BG)
        } else {
            Style::default()
        };
        lines.push(Line::from(header).style(header_style));

        for mut line in crate::text::render_lines(&version.text, &version.formatting.marks, width) {
            if selected {
                line = line.style(Style::default().bg(SELECTED_BG));
            }
            lines.push(line);
        }
        if selected {
            sel_last = lines.len();
        }
        lines.push(Line::from(""));
    }

    frame.render_widget(
        Paragraph::new(lines).scroll((scroll_to_show(sel_first, sel_last, inner.height as usize), 0)),
        inner,
    );
}

/// Smallest scroll offset that keeps rows `first..last` visible in a
/// viewport `height` rows tall. An entry taller than the viewport is
/// pinned to its top rather than its bottom, so you see where it starts.
fn scroll_to_show(first: usize, last: usize, height: usize) -> u16 {
    if height == 0 || last <= height {
        return 0;
    }
    if last - first >= height {
        first as u16
    } else {
        (last - height) as u16
    }
}
