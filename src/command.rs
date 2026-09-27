//! The `:command` registry and parser.
//!
//! Two kinds of commands exist:
//! - Fixed commands in `COMMANDS` below (`:history`, `:restore`, `:new`, …).
//! - Formatting commands, resolved dynamically from `crate::format::FORMATS`
//!   (`:i`, `:b`, `:u`, …) so adding a formatting type automatically adds
//!   its command — no entry needed here.
//!
//! See FEATURES.md "Add a new command" / "Add a command alias".

use crate::app::App;

pub struct CommandDef {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub help: &'static str,
    pub run: fn(&mut App, &[&str]) -> Result<(), String>,
}

pub const COMMANDS: &[CommandDef] = &[
    CommandDef {
        name: "new",
        aliases: &["n"],
        help: ":new — start composing a new entry",
        run: |app, _args| {
            crate::action::apply(app, crate::keybind::Action::NewEntry);
            Ok(())
        },
    },
    CommandDef {
        name: "title",
        aliases: &["name"],
        help: ":title <text> — title the entry you're editing (no text clears it)",
        run: |app, args| crate::action::set_title(app, &args.join(" ")),
    },
    CommandDef {
        name: "link",
        aliases: &["url"],
        help: ":link <url> — make the selection a link (gx opens it)",
        run: |app, args| crate::action::set_link(app, &args.join(" ")),
    },
    CommandDef {
        name: "unlink",
        aliases: &[],
        help: ":unlink — remove the link again",
        run: |app, _args| crate::action::clear_link(app),
    },
    CommandDef {
        name: "done",
        aliases: &[],
        help: ":done — cross out the selection, or the current line",
        run: |app, _args| crate::action::set_done(app, true),
    },
    CommandDef {
        name: "undone",
        aliases: &[],
        help: ":undone — remove the cross-out again",
        run: |app, _args| crate::action::set_done(app, false),
    },
    CommandDef {
        name: "history",
        aliases: &["hist"],
        help: ":history [id] — open the version history of an entry",
        run: |app, args| {
            let id = match args.first() {
                Some(id) => id.to_string(),
                None => app
                    .editor
                    .entry_id
                    .clone()
                    .or_else(|| app.store.entries.get(app.selected).map(|e| e.id.clone()))
                    .ok_or("No entry selected and no ID given")?,
            };
            crate::action::open_history(app, &id);
            Ok(())
        },
    },
    CommandDef {
        name: "restore",
        aliases: &[],
        help: ":restore <version> — restore an old version as a new version",
        run: |app, args| {
            let n: u32 = args
                .first()
                .ok_or("Usage: :restore <version-number>")?
                .parse()
                .map_err(|_| "Version must be a number")?;
            crate::action::restore_version(app, n)
        },
    },
    CommandDef {
        name: "delete",
        aliases: &["d"],
        help: ":delete — delete the selected entry",
        run: |app, _args| {
            crate::action::apply(app, crate::keybind::Action::DeleteSelected);
            Ok(())
        },
    },
    CommandDef {
        name: "w",
        aliases: &["write"],
        help: ":w — save the draft as a new version, stay in the editor",
        run: |app, _args| crate::action::write_editor(app),
    },
    CommandDef {
        name: "wq",
        aliases: &["x", "wq!"],
        help: ":wq — save and close the editor (quits from the journal)",
        run: |app, _args| {
            if matches!(app.mode, crate::app::Mode::Editor) {
                crate::action::apply(app, crate::keybind::Action::EditorSaveExit);
            } else {
                app.should_quit = true;
            }
            Ok(())
        },
    },
    CommandDef {
        name: "quit",
        aliases: &["q"],
        help: ":q — close the editor (refuses unsaved work), or quit the app",
        run: |app, _args| crate::action::quit_context(app, false),
    },
    CommandDef {
        name: "q!",
        aliases: &["quit!"],
        help: ":q! — close the editor discarding changes, or quit the app",
        run: |app, _args| crate::action::quit_context(app, true),
    },
    CommandDef {
        name: "help",
        aliases: &["h"],
        help: ":help — show keybindings and commands",
        run: |app, _args| {
            crate::action::apply(app, crate::keybind::Action::ShowHelp);
            Ok(())
        },
    },
];

pub fn find(name: &str) -> Option<&'static CommandDef> {
    COMMANDS
        .iter()
        .find(|c| c.name == name || c.aliases.contains(&name))
}

/// Parses and runs one `:`-command line (without the leading colon).
pub fn execute(app: &mut App, line: &str) -> Result<(), String> {
    let mut parts = line.split_whitespace();
    let name = parts.next().ok_or("Empty command")?;
    let args: Vec<&str> = parts.collect();

    if let Some(def) = find(name) {
        return (def.run)(app, &args);
    }
    if let Some(fmt) = crate::format::find_by_command(name) {
        return crate::action::toggle_format(app, fmt.kind);
    }
    Err(format!("Unknown command: {name}"))
}
