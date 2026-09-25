//! The read-only, tab-per-version history view (`:history <id>`).
//!
//! Tab hit-test rects are recorded into `App::history_tab_cols` every
//! frame so mouse clicks can select a tab — see `main.rs`'s mouse event
//! handling, which is the only other place that needs to know about it.

use crate::app::App;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect, entry_id: &str, selected: usize) {
    let Some(entry) = app.store.find(entry_id) else {
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" History — {entry_id}   READ ONLY "));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [tabs_area, content_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);

    // Build tab labels, then show a window of them that always includes
    // the selected one - with many versions they can't all fit, and a
    // selected tab scrolled off-screen would leave nothing indicating
    // which version you're looking at.
    let labels: Vec<String> = entry
        .versions
        .iter()
        .map(|v| {
            format!(
                " [{}] {}{} ",
                v.version_number,
                v.created_at.format("%H:%M:%S"),
                if v.restored_from.is_some() { "*" } else { "" }
            )
        })
        .collect();
    let widths: Vec<usize> = labels.iter().map(|l| l.chars().count()).collect();

    // One column each is reserved for the ‹ / › "more tabs" markers, so
    // the recorded click columns below line up with what's drawn.
    let total = labels.len();
    let mut first = 0usize;
    loop {
        let left_marker = usize::from(first > 0);
        let avail = (tabs_area.width as usize).saturating_sub(left_marker + 1);
        if first >= selected || widths[first..=selected].iter().sum::<usize>() <= avail {
            break;
        }
        first += 1;
    }
    let left_marker = first > 0;
    let avail = (tabs_area.width as usize).saturating_sub(usize::from(left_marker) + 1);

    app.history_tab_cols.clear();
    app.history_tab_row = tabs_area.y;
    let mut spans = Vec::new();
    let mut col = tabs_area.x;
    if left_marker {
        spans.push(Span::styled("‹", Style::default().fg(Color::Cyan)));
        col += 1;
    }

    let mut used = 0usize;
    let mut last = first;
    for (idx, label) in labels.iter().enumerate().skip(first) {
        if used + widths[idx] > avail && idx != selected {
            break;
        }
        let style = if idx == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let w = widths[idx] as u16;
        app.history_tab_cols.push((col, col + w, idx));
        spans.push(Span::styled(label.clone(), style));
        col += w;
        used += widths[idx];
        last = idx;
    }
    if last + 1 < total {
        spans.push(Span::styled("›", Style::default().fg(Color::Cyan)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), tabs_area);

    let Some(version) = entry.versions.get(selected) else {
        return;
    };
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "Version {} / {}   {}{}",
            version.version_number,
            entry.versions.len(),
            version.created_at.format("%d/%m/%Y %H:%M:%S"),
            match version.restored_from {
                Some(n) => format!("  (restored from version {n})"),
                None => String::new(),
            }
        ),
        Style::default().fg(Color::Yellow),
    ))];
    // The title is versioned too, so show the one this version carried
    // rather than the entry's current one.
    if let Some(t) = &version.title {
        lines.push(Line::from(Span::styled(
            format!("Title: {t}"),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.extend(crate::text::render_lines(
        &version.text,
        &version.formatting.marks,
        content_area.width as usize,
    ));

    frame.render_widget(Paragraph::new(lines), content_area);
}
