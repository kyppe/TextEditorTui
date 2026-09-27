//! Generic floating popups, drawn over whatever the base view rendered.
//!
//! To add a new popup: write a `pub fn my_popup(frame: &mut Frame, ...)`
//! here following the pattern below, add a `Mode` variant that carries
//! whatever data it needs plus a `return_to: Box<Mode>`, and call it
//! from the `match &app.mode` in `ui/mod.rs::draw`. See FEATURES.md
//! "Add a new UI popup".

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Centers a box of `width` x `height` within `area`.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}

pub fn confirm(frame: &mut Frame, message: &str) {
    let width = (message.chars().count() as u16 + 4).clamp(34, 60);
    let width = width.min(frame.area().width);
    let inner_width = width.saturating_sub(2) as usize;

    // Pre-wrapped with the project's own wrapper rather than ratatui's
    // `Wrap`, so the row count below is exact — a two-line message used to
    // push the y/n hint out of a fixed-height box.
    let mut lines = crate::text::render_lines(message, &[], inner_width);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "y / Enter: yes      n / Esc: no",
        Style::default().fg(Color::DarkGray),
    )));

    let height = (lines.len() as u16 + 2).min(frame.area().height);
    let area = centered(frame.area(), width, height);

    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// The `t` popup: shows the entry's existing title for editing, or an
/// empty field to name it for the first time.
pub fn title_prompt(frame: &mut Frame, entry_id: &str, input: &str, had_title: bool) {
    let width = 54.min(frame.area().width);
    let area = centered(frame.area(), width, 6);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(if had_title {
            " Edit title "
        } else {
            " Name this entry "
        })
        .style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(
            format!("Entry {entry_id}"),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Magenta)),
            Span::styled(
                input.to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: save   Esc: cancel   (empty clears the title)",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The `:goto` picker: every entry you could cross-reference, filtered by
/// what you've typed, with one row highlighted. Rows come from
/// `App::goto_candidates`, the same function the Enter handler uses, so what
/// is highlighted here is what gets linked.
pub fn goto_prompt(frame: &mut Frame, app: &crate::app::App, query: &str, highlighted: usize) {
    let candidates = app.goto_candidates(query);

    let width = 64.min(frame.area().width);
    // Rows, plus the query line, the separator, a blank and the key hint,
    // plus the two border rows.
    let rows = candidates.len().clamp(1, 10) as u16;
    let height = (rows + 6).min(frame.area().height);
    let area = centered(frame.area(), width, height);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Link to which entry? ")
        .style(Style::default().fg(Color::Rgb(126, 224, 205)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("filter ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                query.to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Color::Rgb(126, 224, 205))),
        ]),
        Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if candidates.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no entry matches",
            Style::default().fg(Color::Red),
        )));
    }

    // Scroll the row window so the highlighted row stays visible.
    let visible = rows as usize;
    let first = highlighted.saturating_sub(visible.saturating_sub(1));
    for (offset, index) in candidates.iter().enumerate().skip(first).take(visible) {
        let entry = &app.store.entries[*index];
        let chosen = offset == highlighted;
        let marker = if chosen { "▸ " } else { "  " };
        let row_style = if chosen {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(126, 224, 205))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        // Truncate the label so a long first line can't push the id off.
        let room = inner.width as usize;
        let prefix = format!("{marker}[{}] ", entry.id);
        let label: String = entry
            .label()
            .chars()
            .take(room.saturating_sub(prefix.chars().count() + 1))
            .collect();
        lines.push(Line::from(Span::styled(
            format!("{prefix}{label}"),
            row_style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "type to filter · ↑/↓ choose · Enter link · Esc cancel",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Draws the cheat sheet. Returns the largest useful scroll offset, which
/// the caller stores on `App` so the scroll keys can be clamped to the
/// content actually rendered at this terminal size.
pub fn help(frame: &mut Frame, scroll: u16) -> u16 {
    // Near-full-window box: this is a cheat sheet, it should use the room it
    // has. Width is fixed up front so the content can be pre-wrapped to it.
    let width = 60.min(frame.area().width);
    let inner_width = width.saturating_sub(2) as usize;

    let heading = |s: &'static str| Line::from(Span::styled(s, Style::default().fg(Color::Cyan)));
    // Long entries are wrapped with the project's own wrapper rather than
    // ratatui's `Wrap`, so `lines.len()` equals the rows actually drawn.
    // With `Wrap` doing it, the scroll limit below undercounted and the
    // tail of the list became unreachable.
    let body = |s: &str| crate::text::render_lines(s, &[], inner_width);

    let mut lines = vec![heading("Journal")];
    for l in [
        "  j/k, ↑/↓   move selection",
        "  n, o       new entry",
        "  i, Enter   edit selected entry",
        "  d          delete selected entry (confirms)",
        "  H          open history for selected entry",
        "  :          command line      ?  this help",
        "  q          quit",
        "",
    ] {
        lines.extend(body(l));
    }
    lines.push(heading("Editor"));
    for l in [
        "  i / a      start typing",
        "  Esc        stop typing, then Esc again to exit",
        "  h/l/j/k, arrows  move cursor   0 / $  line start/end",
        "  v          start/clear selection, then : + a format cmd",
        "  :title T   title this entry",
        "  Ctrl+S     save (as a new version) and exit",
        "",
    ] {
        lines.extend(body(l));
    }
    lines.push(heading("History — read-only"));
    for l in [
        "  h/l, ←/→, Tab   switch version (or click a tab)",
        "  Ctrl+1..9       jump to version N",
        "  Esc, q          close",
        "",
    ] {
        lines.extend(body(l));
    }

    // Built from the registries, so a newly added command or formatting
    // type shows up here with no extra work - see FEATURES.md.
    lines.push(heading("Commands"));
    for c in crate::command::COMMANDS {
        lines.extend(body(&format!("  {}", c.help)));
    }
    lines.push(heading("Formatting — applies to the selection"));
    for f in crate::format::FORMATS {
        // Marks without a plain toggle command (Link) are documented by
        // their own command above instead.
        if let Some(cmd) = f.command {
            lines.extend(body(&format!("  :{:<6} {}", cmd, f.name)));
        }
    }

    let height = (lines.len() as u16 + 2).min(frame.area().height);
    let area = centered(frame.area(), width, height);
    let inner_height = area.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(inner_height) as u16;
    let scroll = scroll.min(max_scroll);

    let title = if max_scroll > 0 {
        " Help — j/k scroll, any other key closes "
    } else {
        " Help — press any key to close "
    };

    frame.render_widget(Clear, area);
    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll, 0)), area);
    max_scroll
}
