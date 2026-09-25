//! The composing/editing view: a plain-text buffer with live formatting
//! marks, an optional selection highlight, and a terminal cursor.
//!
//! Text soft-wraps to the window width and the view scrolls vertically to
//! keep the cursor on screen. Both come from `text::render_lines_sel` /
//! `text::visual_row_col`, which share one wrap calculation — so the
//! cursor always sits on the character it's actually editing.

use crate::app::App;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let title = match (&app.editor.entry_id, &app.editor.title) {
        (Some(id), Some(t)) => format!(" Editing {id} — {t} "),
        (Some(id), None) => format!(" Editing {id} "),
        (None, Some(t)) => format!(" New entry — {t} "),
        (None, None) => " New entry ".to_string(),
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Line-number gutter, sized to the widest number it has to show.
    let logical_lines = app.editor.text.split('\n').count();
    let gutter = (logical_lines.to_string().len() + 1) as u16;
    let [gutter_area, text_area] =
        Layout::horizontal([Constraint::Length(gutter), Constraint::Min(1)]).areas(inner);
    let width = text_area.width as usize;
    if width == 0 {
        return;
    }

    let lines = crate::text::render_lines_sel(
        &app.editor.text,
        &app.editor.formatting.marks,
        width,
        app.editor.selection_range(),
    );

    let (cursor_row, cursor_col) =
        crate::text::visual_row_col(&app.editor.text, app.editor.cursor, width);
    // Scroll the minimum needed to keep the cursor row inside the view.
    let scroll = cursor_row.saturating_sub(text_area.height.saturating_sub(1));

    frame.render_widget(
        Paragraph::new(numbers(&app.editor.text, width, gutter, cursor_row)).scroll((scroll, 0)),
        gutter_area,
    );
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), text_area);

    // Only claim the terminal cursor when this view is the thing being
    // typed into: with the `:` command line open, the cursor belongs there.
    if matches!(app.mode, crate::app::Mode::Editor) {
        frame.set_cursor_position((text_area.x + cursor_col, text_area.y + cursor_row - scroll));
    }
}

/// One gutter entry per *visual* row: the logical line number on the row
/// where that line starts, blank on its soft-wrapped continuation rows
/// (the same thing Vim does with `wrap` on). The cursor's line is
/// highlighted so it's findable at a glance.
fn numbers(text: &str, width: usize, gutter: u16, cursor_row: u16) -> Vec<Line<'static>> {
    let breaks = crate::text::wrap_positions(text, width);
    let pad = gutter.saturating_sub(1) as usize;
    let mut out = Vec::new();
    let mut next_break = 0usize;
    let mut line_no = 1usize;
    let mut fresh_line = true;

    let push = |n: Option<usize>, out: &mut Vec<Line<'static>>| {
        let label = match n {
            Some(n) => format!("{n:>pad$} "),
            None => " ".repeat(pad + 1),
        };
        let style = if out.len() as u16 == cursor_row {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        out.push(Line::from(Span::styled(label, style)));
    };

    for (i, ch) in text.chars().enumerate() {
        while next_break < breaks.len() && breaks[next_break] == i {
            push(if fresh_line { Some(line_no) } else { None }, &mut out);
            fresh_line = false;
            next_break += 1;
        }
        if ch == '\n' {
            push(if fresh_line { Some(line_no) } else { None }, &mut out);
            line_no += 1;
            fresh_line = true;
        }
    }
    push(if fresh_line { Some(line_no) } else { None }, &mut out);
    out
}
