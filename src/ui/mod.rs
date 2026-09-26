//! Rendering. `draw` dispatches on `App::mode`; each mode's content view
//! lives in its own module. Popups (`Confirm`, `Help`, the `:` command
//! line) are drawn as an overlay on top of whatever the mode underneath
//! (`return_to`) would have shown — see `base_mode`.
//!
//! See FEATURES.md "Add a new UI popup" for the pattern to copy.

mod editor;
mod history;
mod normal;
mod popup;

use crate::app::{App, Mode};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Unwraps overlay modes (`Command`, `Confirm`, `Help`) to find the mode
/// whose content should be drawn underneath the overlay.
fn base_mode(mode: &Mode) -> &Mode {
    match mode {
        Mode::Command { return_to } => base_mode(return_to),
        Mode::Confirm { return_to, .. } => base_mode(return_to),
        Mode::Help { return_to, .. } => base_mode(return_to),
        Mode::TitlePrompt { return_to, .. } => base_mode(return_to),
        other => other,
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [content_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

    match base_mode(&app.mode).clone() {
        Mode::Normal => normal::draw(frame, app, content_area),
        Mode::Editor => editor::draw(frame, app, content_area),
        Mode::History {
            entry_id, selected, ..
        } => {
            history::draw(frame, app, content_area, &entry_id, selected)
        }
        _ => unreachable!("base_mode never returns an overlay mode"),
    }

    draw_status(frame, app, status_area);

    match app.mode.clone() {
        Mode::Confirm { message, .. } => popup::confirm(frame, &message),
        Mode::Help { scroll, .. } => {
            app.help_max_scroll = popup::help(frame, scroll);
        }
        Mode::TitlePrompt {
            entry_id, input, ..
        } => {
            let had_title = app
                .store
                .find(&entry_id)
                .is_some_and(|e| e.title().is_some());
            popup::title_prompt(frame, &entry_id, &input, had_title);
        }
        _ => {}
    }
}

/// Which mode the bottom-left badge announces, and its colour. The
/// editor's three Vim-ish states (NORMAL / INSERT / VISUAL) are the
/// distinction that matters most while typing; the other screens name
/// themselves so it's clear which keymap is in effect.
fn mode_badge(app: &App) -> (&'static str, Color) {
    match &app.mode {
        Mode::Command { .. } => ("COMMAND", Color::Yellow),
        Mode::History { .. } => ("HISTORY", Color::Cyan),
        Mode::Help { .. } => ("HELP", Color::Cyan),
        Mode::Confirm { .. } => ("CONFIRM", Color::Red),
        Mode::TitlePrompt { .. } => ("TITLE", Color::Magenta),
        Mode::Normal => ("JOURNAL", Color::Blue),
        Mode::Editor => {
            if app.editor.typing {
                ("INSERT", Color::Green)
            } else if app.editor.selection_anchor.is_some() {
                ("VISUAL", Color::Magenta)
            } else {
                ("NORMAL", Color::Blue)
            }
        }
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let (label, color) = mode_badge(app);
    let mut spans = vec![
        Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    if let Mode::Command { .. } = &app.mode {
        spans.push(Span::styled(":", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(app.command_input.clone()));
        spans.push(Span::styled("█", Style::default().fg(Color::Cyan)));
    } else if let Some((msg, level)) = &app.status {
        let msg_color = match level {
            crate::app::StatusLevel::Info => Color::Green,
            crate::app::StatusLevel::Error => Color::Red,
        };
        spans.push(Span::styled(
            msg.clone(),
            Style::default().fg(msg_color),
        ));
    } else if let Some(c) = app.pending_key {
        // A half-typed chord is worth showing, so `g` doesn't look like a no-op.
        spans.push(Span::styled(
            format!("{c}…"),
            Style::default().fg(Color::Yellow),
        ));
    } else {
        let hint = match base_mode(&app.mode) {
            Mode::Normal => "j/k gg/G move · i edit · n new · t title · d delete · H history · ? help",
            Mode::Editor => {
                if app.editor.typing {
                    "Esc: normal mode · Tab: indent · Ctrl+S: save & exit"
                } else if app.editor.selection_anchor.is_some() {
                    "y yank · d/x cut · c change · p paste over · :done · Esc"
                } else {
                    "i a o O · w b e gg G · x dd dw cw D C · yy p P · u Ctrl+r · :w :wq :q"
                }
            }
            Mode::History { .. } => {
                "h/l ←/→ Tab switch · gg/G first/last · Ctrl+1-9 jump · :restore N · Esc"
            }
            _ => "",
        };
        spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
