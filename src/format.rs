//! The formatting/mark registry: the single place that knows which
//! `:command` and which render style corresponds to each `MarkKind`.
//!
//! To add a new formatting type, see FEATURES.md "Add a new formatting
//! type" — you add one `MarkKind` variant in `entry.rs` and one entry in
//! `FORMATS` below. No other file needs to change: the command parser,
//! the editor, and both renderers (list view and editor view) all read
//! from this table.

use crate::entry::MarkKind;
use ratatui::style::{Color, Modifier, Style};

pub struct FormatDef {
    pub kind: MarkKind,
    /// The `:` command name that toggles this mark, e.g. `"i"` for `:i`.
    /// `None` for marks a plain toggle can't express — `Link` needs a URL,
    /// so it has its own `:link` command in `command.rs` instead.
    pub command: Option<&'static str>,
    /// Human-readable name shown in help text.
    pub name: &'static str,
    pub style: fn() -> Style,
}

pub const FORMATS: &[FormatDef] = &[
    FormatDef {
        kind: MarkKind::Italic,
        command: Some("i"),
        name: "italic",
        style: || Style::default().add_modifier(Modifier::ITALIC),
    },
    FormatDef {
        kind: MarkKind::Bold,
        command: Some("b"),
        name: "bold",
        style: || Style::default().add_modifier(Modifier::BOLD),
    },
    FormatDef {
        kind: MarkKind::Underline,
        command: Some("u"),
        name: "underline",
        style: || Style::default().add_modifier(Modifier::UNDERLINED),
    },
    FormatDef {
        kind: MarkKind::Code,
        command: Some("code"),
        name: "code",
        style: || Style::default().fg(Color::Yellow),
    },
    FormatDef {
        kind: MarkKind::Highlight,
        command: Some("hl"),
        name: "highlight",
        style: || {
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
        },
    },
    FormatDef {
        kind: MarkKind::Strikethrough,
        // `:done` / `:undone` (see command.rs) are the line-aware way to
        // apply this; this entry keeps the plain selection toggle available
        // and is what tells every renderer how a struck run looks.
        command: Some("strike"),
        name: "strikethrough",
        style: || Style::default().add_modifier(Modifier::CROSSED_OUT),
    },
    FormatDef {
        kind: MarkKind::Link,
        // `:link` needs a URL argument, so it has its own command in
        // command.rs; this entry is what tells the renderers how a link
        // looks. (`:h` is Vim's help, so it isn't used for links here.)
        command: None,
        name: "link",
        // A light blue rather than the terminal's dark blue: links have to
        // stay legible on the journal list's highlighted-entry background
        // as well as on the default one.
        style: || {
            Style::default()
                .fg(Color::Rgb(122, 176, 255))
                .add_modifier(Modifier::UNDERLINED)
        },
    },
    FormatDef {
        kind: MarkKind::Heading,
        command: Some("head"),
        name: "heading",
        style: || {
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        },
    },
];

pub fn find_by_command(cmd: &str) -> Option<&'static FormatDef> {
    FORMATS.iter().find(|f| f.command == Some(cmd))
}

pub fn find_by_kind(kind: MarkKind) -> &'static FormatDef {
    FORMATS
        .iter()
        .find(|f| f.kind == kind)
        .expect("every MarkKind variant must have a FORMATS entry")
}
