//! Editor actions: the only code that actually mutates `App` state in
//! response to something the user did. Both `keybind::resolve` (a key
//! press) and `command::execute` (a `:command`) end in a call to
//! `apply` or to one of the `pub fn`s below — so a new command and a
//! new shortcut that do the same thing share one implementation.
//!
//! See FEATURES.md "Add a new editor action" before adding a variant.

use crate::app::{App, ConfirmAction, Mode};
use crate::entry::{Formatting, Mark, MarkKind};
use crate::keybind::Action;
use crate::text;

pub fn apply(app: &mut App, action: Action) {
    app.status = None;
    // A chord's first key is the only thing that leaves `pending_key` set;
    // every other action consumes and clears it.
    if matches!(action, Action::SetPending(_)) {
        if let Action::SetPending(c) = action {
            app.pending_key = Some(c);
        }
        return;
    }
    app.pending_key = None;

    match action {
        Action::NoOp => {}
        Action::SetPending(_) => unreachable!("handled above"),
        Action::RequestQuit => app.should_quit = true,

        // Normal mode
        Action::SelectUp => {
            if app.selected > 0 {
                app.selected -= 1;
            }
        }
        Action::SelectDown => {
            if app.selected + 1 < app.store.entries.len() {
                app.selected += 1;
            }
        }
        Action::SelectFirst => app.selected = 0,
        Action::SelectLast => app.selected = app.store.entries.len().saturating_sub(1),
        Action::OpenTitlePrompt => open_title_prompt(app),
        Action::NewEntry => new_entry(app),
        Action::EditSelected => edit_selected(app),
        Action::DeleteSelected => delete_selected_prompt(app),
        Action::OpenHistorySelected => open_history_selected(app),
        Action::EnterCommand => enter_command(app),
        Action::ShowHelp => {
            app.mode = Mode::Help {
                return_to: Box::new(app.mode.clone()),
                scroll: 0,
            }
        }

        // Editor, normal sub-mode
        Action::EditorEnterTyping => app.editor.typing = true,
        Action::EditorAppendTyping => {
            app.editor.cursor = (app.editor.cursor + 1).min(text::char_len(&app.editor.text));
            app.editor.typing = true;
        }
        Action::EditorMoveLeft => move_left(app),
        Action::EditorMoveRight => move_right(app),
        Action::EditorMoveUp => move_vertical(app, -1),
        Action::EditorMoveDown => move_vertical(app, 1),
        Action::EditorLineStart => {
            let (row, _) = text::row_col(&app.editor.text, app.editor.cursor);
            app.editor.cursor = text::char_index_at(&app.editor.text, row, 0);
        }
        Action::EditorLineEnd => {
            let (row, _) = text::row_col(&app.editor.text, app.editor.cursor);
            let len = text::current_line_len(&app.editor.text, app.editor.cursor);
            app.editor.cursor = text::char_index_at(&app.editor.text, row, len as u16);
        }
        Action::EditorToggleSelection => {
            if app.editor.selection_anchor.is_some() {
                app.editor.selection_anchor = None;
                app.set_info("Selection cleared");
            } else {
                app.editor.selection_anchor = Some(app.editor.cursor);
                app.set_info("Selection started — move to extend, : to format");
            }
        }
        // Vim motions
        Action::EditorBufferStart => app.editor.cursor = 0,
        Action::EditorBufferEnd => {
            app.editor.cursor = text::last_line_start(&app.editor.text);
        }
        Action::EditorWordForward => {
            app.editor.cursor = text::next_word_start(&app.editor.text, app.editor.cursor);
        }
        Action::EditorWordBack => {
            app.editor.cursor = text::prev_word_start(&app.editor.text, app.editor.cursor);
        }
        Action::EditorWordEnd => {
            app.editor.cursor = text::word_end(&app.editor.text, app.editor.cursor);
        }
        Action::EditorFirstNonBlank => {
            app.editor.cursor = text::first_non_blank(&app.editor.text, app.editor.cursor);
        }

        // Vim edits
        Action::EditorDeleteChar => {
            let len = text::char_len(&app.editor.text);
            if app.editor.cursor < len {
                delete_range(app, app.editor.cursor, app.editor.cursor + 1);
            }
        }
        Action::EditorDeleteToLineEnd => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            delete_range(app, app.editor.cursor, end);
        }
        Action::EditorDeleteLine => {
            let (start, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            let len = text::char_len(&app.editor.text);
            // Take the trailing newline with the line; on the last line take
            // the preceding one instead, so no blank line is left behind.
            let (from, to) = if end < len {
                (start, end + 1)
            } else if start > 0 {
                (start - 1, end)
            } else {
                (start, end)
            };
            delete_range(app, from, to);
            app.editor.cursor = from.min(text::char_len(&app.editor.text));
        }
        Action::EditorOpenLineBelow => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            app.editor.cursor = end;
            apply(app, Action::EditorNewline);
            app.editor.typing = true;
        }
        Action::EditorOpenLineAbove => {
            let (start, _) = text::line_bounds(&app.editor.text, app.editor.cursor);
            app.editor.cursor = start;
            text::insert_char(&mut app.editor.text, start, '\n');
            shift_marks_for_insert(&mut app.editor.formatting, start, 1);
            app.editor.cursor = start;
            app.editor.typing = true;
        }
        Action::EditorInsertAtLineStart => {
            app.editor.cursor = text::first_non_blank(&app.editor.text, app.editor.cursor);
            app.editor.typing = true;
        }
        Action::EditorAppendAtLineEnd => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            app.editor.cursor = end;
            app.editor.typing = true;
        }

        Action::EditorSelectLine => {
            let (start, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            app.editor.selection_anchor = Some(start);
            app.editor.cursor = end;
            app.set_info("Line selected — : then a format command, or :done");
        }
        Action::EditorSaveExit => save_editor(app),
        Action::EditorCancelExit => cancel_editor(app),
        Action::EditorOpenHistory => {
            if let Some(id) = app.editor.entry_id.clone() {
                open_history(app, &id);
            } else {
                app.set_error("This entry hasn't been saved yet");
            }
        }

        // Editor, typing sub-mode
        Action::EditorInsertChar(c) => {
            text::insert_char(&mut app.editor.text, app.editor.cursor, c);
            shift_marks_for_insert(&mut app.editor.formatting, app.editor.cursor, 1);
            app.editor.cursor += 1;
        }
        Action::EditorBackspace => {
            if app.editor.cursor > 0 {
                text::remove_before(&mut app.editor.text, app.editor.cursor);
                shift_marks_for_delete(&mut app.editor.formatting, app.editor.cursor - 1, 1);
                app.editor.cursor -= 1;
            }
        }
        Action::EditorNewline => {
            text::insert_char(&mut app.editor.text, app.editor.cursor, '\n');
            shift_marks_for_insert(&mut app.editor.formatting, app.editor.cursor, 1);
            app.editor.cursor += 1;
        }
        Action::EditorExitTyping => app.editor.typing = false,

        // Command line
        Action::CommandInsertChar(c) => app.command_input.push(c),
        Action::CommandBackspace => {
            app.command_input.pop();
        }
        Action::CommandSubmit => submit_command(app),
        Action::CommandCancel => {
            if let Mode::Command { return_to } = &app.mode {
                app.mode = (**return_to).clone();
            }
            app.command_input.clear();
        }

        // History view
        Action::HistoryPrev => history_step(app, -1),
        Action::HistoryNext => history_step(app, 1),
        Action::HistoryFirst => history_goto(app, 0),
        Action::HistoryLast => {
            if let Mode::History { entry_id, .. } = app.mode.clone() {
                let last = app
                    .store
                    .find(&entry_id)
                    .map(|e| e.versions.len().saturating_sub(1))
                    .unwrap_or(0);
                history_goto(app, last);
            }
        }
        Action::HistoryGoto(idx) => history_goto(app, idx),
        Action::HistoryClose => {
            if let Mode::History { return_to, .. } = &app.mode {
                app.mode = (**return_to).clone();
                if matches!(app.mode, Mode::Editor) {
                    app.set_info("Back to your draft — Ctrl+S to save");
                }
            }
        }
        Action::HistoryEnterCommand => enter_command(app),

        // Confirm popup
        Action::ConfirmYes => confirm_yes(app),
        Action::ConfirmNo => {
            if let Mode::Confirm { return_to, .. } = &app.mode {
                app.mode = (**return_to).clone();
            }
        }

        Action::HelpScrollDown => {
            if let Mode::Help { return_to, scroll } = &app.mode {
                app.mode = Mode::Help {
                    return_to: return_to.clone(),
                    scroll: (scroll + 1).min(app.help_max_scroll),
                };
            }
        }
        Action::HelpScrollUp => {
            if let Mode::Help { return_to, scroll } = &app.mode {
                app.mode = Mode::Help {
                    return_to: return_to.clone(),
                    scroll: scroll.saturating_sub(1),
                };
            }
        }
        Action::HelpClose => {
            if let Mode::Help { return_to, .. } = &app.mode {
                app.mode = (**return_to).clone();
            }
        }

        // Title popup
        Action::TitleInsertChar(c) => {
            if let Mode::TitlePrompt { input, .. } = &mut app.mode {
                input.push(c);
            }
        }
        Action::TitleBackspace => {
            if let Mode::TitlePrompt { input, .. } = &mut app.mode {
                input.pop();
            }
        }
        Action::TitleSubmit => submit_title_prompt(app),
        Action::TitleCancel => {
            if let Mode::TitlePrompt { return_to, .. } = &app.mode {
                app.mode = (**return_to).clone();
            }
        }
    }
}

fn move_left(app: &mut App) {
    app.editor.cursor = app.editor.cursor.saturating_sub(1);
}

fn move_right(app: &mut App) {
    app.editor.cursor = (app.editor.cursor + 1).min(text::char_len(&app.editor.text));
}

fn move_vertical(app: &mut App, delta: i32) {
    let (row, col) = text::row_col(&app.editor.text, app.editor.cursor);
    let target_row = row as i32 + delta;
    if target_row < 0 {
        return;
    }
    app.editor.cursor = text::char_index_at(&app.editor.text, target_row as u16, col);
}

/// Shifts every mark boundary at or after `at` forward by `count`
/// characters, so formatting stays attached to the same text after an
/// insert earlier in the buffer.
fn shift_marks_for_insert(formatting: &mut Formatting, at: usize, count: usize) {
    for m in &mut formatting.marks {
        if m.start >= at {
            m.start += count;
        }
        if m.end >= at {
            m.end += count;
        }
    }
}

/// Moves mark boundaries back over a deletion of `[at, at + count)`.
/// Boundaries *inside* the deleted span collapse onto `at` rather than
/// being shifted past it — which matters now that whole lines can go at
/// once (`dd`, `D`), not just single characters.
fn shift_marks_for_delete(formatting: &mut Formatting, at: usize, count: usize) {
    let end = at + count;
    let shift = |p: usize| {
        if p <= at {
            p
        } else if p >= end {
            p - count
        } else {
            at
        }
    };
    for m in &mut formatting.marks {
        m.start = shift(m.start);
        m.end = shift(m.end);
    }
    formatting.marks.retain(|m| m.end > m.start);
}

/// Deletes `[start, end)` from the draft, keeping marks and the cursor in
/// step. The one place multi-character deletion happens.
fn delete_range(app: &mut App, start: usize, end: usize) {
    if start >= end {
        return;
    }
    text::remove_range(&mut app.editor.text, start, end);
    shift_marks_for_delete(&mut app.editor.formatting, start, end - start);
    if app.editor.cursor > start {
        app.editor.cursor = start.max(app.editor.cursor.saturating_sub(end - start));
    }
    app.editor.cursor = app.editor.cursor.min(text::char_len(&app.editor.text));
    // A selection anchored into deleted text would now point at the wrong
    // characters, so drop it rather than silently mis-highlighting.
    app.editor.selection_anchor = None;
}

fn new_entry(app: &mut App) {
    app.editor = crate::app::EditorState::empty();
    app.mode = Mode::Editor;
    app.set_info("New entry — Esc to stop typing, Ctrl+S to save, Esc again to exit");
}

fn edit_selected(app: &mut App) {
    let Some(entry) = app.store.entries.get(app.selected) else {
        return;
    };
    let current = entry.current();
    app.editor = crate::app::EditorState {
        entry_id: Some(entry.id.clone()),
        title: current.title.clone(),
        text: current.text.clone(),
        cursor: text::char_len(&current.text),
        selection_anchor: None,
        formatting: current.formatting.clone(),
        typing: true,
    };
    app.mode = Mode::Editor;
}

fn save_editor(app: &mut App) {
    let text = app.editor.text.clone();
    let formatting = app.editor.formatting.clone();
    let title = app.editor.title.clone();

    if let Some(id) = app.editor.entry_id.clone() {
        // Every part of a version counts as a change, title included -
        // otherwise retitling alone would be silently thrown away.
        let unchanged = app
            .store
            .find(&id)
            .map(|e| {
                let v = e.current();
                v.text == text && v.formatting == formatting && v.title == title
            })
            .unwrap_or(false);
        if !unchanged {
            if let Some(entry) = app.store.find_mut(&id) {
                entry.push_version(title, text, formatting);
            }
            app.set_info("Saved new version");
        } else {
            app.set_info("No changes");
        }
    } else if !text.trim().is_empty() || title.is_some() {
        let id = app.store.unique_id();
        let entry = crate::entry::Entry::new(id, title, text, formatting);
        app.store.entries.push(entry);
        app.selected = app.store.entries.len() - 1;
        app.set_info("Entry saved");
    }
    app.save_store();
    leave_editor(app);
}

fn cancel_editor(app: &mut App) {
    if app.editor.typing {
        app.editor.typing = false;
        return;
    }
    if app.editor.selection_anchor.is_some() {
        app.editor.selection_anchor = None;
        app.set_info("Selection cleared — Esc again to exit without saving");
        return;
    }
    leave_editor(app);
    app.set_info("Left editor — unsaved changes discarded");
}

/// Returns to the journal and drops the editing buffer, so no stale text
/// or selection can be acted on from outside the editor.
fn leave_editor(app: &mut App) {
    app.editor = crate::app::EditorState::empty();
    app.mode = Mode::Normal;
}

fn delete_selected_prompt(app: &mut App) {
    // Deletion targets the *journal* selection, which isn't necessarily the
    // entry open in the editor (composing a new entry, it never is), so only
    // offer it from the journal view.
    if !matches!(app.mode, Mode::Normal) {
        app.set_error("Deleting only works from the journal view (Esc first)");
        return;
    }
    let Some(entry) = app.store.entries.get(app.selected) else {
        return;
    };
    app.mode = Mode::Confirm {
        message: format!("Delete entry {}? This cannot be undone.", entry.id),
        action: ConfirmAction::DeleteEntry(entry.id.clone()),
        return_to: Box::new(Mode::Normal),
    };
}

fn confirm_yes(app: &mut App) {
    let Mode::Confirm { action, .. } = app.mode.clone() else {
        return;
    };
    match action {
        ConfirmAction::DeleteEntry(id) => {
            if let Some(idx) = app.store.index_of(&id) {
                app.store.entries.remove(idx);
                app.selected = app.selected.min(app.store.entries.len().saturating_sub(1));
                app.save_store();
                app.set_info("Entry deleted");
            }
            app.mode = Mode::Normal;
        }
    }
}

fn enter_command(app: &mut App) {
    app.command_input.clear();
    app.mode = Mode::Command {
        return_to: Box::new(app.mode.clone()),
    };
}

fn submit_command(app: &mut App) {
    let Mode::Command { return_to } = app.mode.clone() else {
        return;
    };
    let line = app.command_input.clone();
    app.command_input.clear();
    app.mode = *return_to;
    if !line.trim().is_empty() {
        if let Err(e) = crate::command::execute(app, &line) {
            app.set_error(e);
        }
    }
}

pub fn open_history_selected(app: &mut App) {
    let Some(entry) = app.store.entries.get(app.selected) else {
        return;
    };
    let id = entry.id.clone();
    open_history(app, &id);
}

pub fn open_history(app: &mut App, id: &str) {
    let Some(entry) = app.store.find(id) else {
        app.set_error(format!("No entry with ID {id}"));
        return;
    };
    let selected = entry.versions.len() - 1;
    // Opening history from within history (`:history <other-id>`) must not
    // nest return targets, or Esc would take several presses to get out.
    let return_to = match &app.mode {
        Mode::History { return_to, .. } => return_to.clone(),
        other => Box::new(other.clone()),
    };
    app.mode = Mode::History {
        entry_id: id.to_string(),
        selected,
        return_to,
    };
}

fn history_step(app: &mut App, delta: i32) {
    let Mode::History {
        entry_id,
        selected,
        return_to,
    } = app.mode.clone()
    else {
        return;
    };
    let Some(entry) = app.store.find(&entry_id) else {
        return;
    };
    let count = entry.versions.len() as i32;
    let next = (selected as i32 + delta).clamp(0, count - 1) as usize;
    app.mode = Mode::History {
        entry_id,
        selected: next,
        return_to,
    };
}

pub fn history_goto(app: &mut App, idx: usize) {
    let Mode::History {
        entry_id,
        return_to,
        ..
    } = app.mode.clone()
    else {
        return;
    };
    let Some(entry) = app.store.find(&entry_id) else {
        return;
    };
    if idx < entry.versions.len() {
        app.mode = Mode::History {
            entry_id,
            selected: idx,
            return_to,
        };
    }
}

pub fn restore_version(app: &mut App, version_number: u32) -> Result<(), String> {
    let Mode::History {
        entry_id,
        return_to,
        ..
    } = app.mode.clone()
    else {
        return Err("`:restore` only works inside a history view".into());
    };
    let Some(entry) = app.store.find_mut(&entry_id) else {
        return Err(format!("No entry with ID {entry_id}"));
    };
    if !entry.restore(version_number) {
        return Err(format!("No version {version_number} on this entry"));
    }
    let new_selected = entry.versions.len() - 1;
    app.save_store();
    // Restoring changes the entry's current version, so a draft in the
    // editor is now based on stale content - go back to the journal.
    let return_to = match *return_to {
        Mode::Editor => Box::new(Mode::Normal),
        other => Box::new(other),
    };
    app.mode = Mode::History {
        entry_id,
        selected: new_selected,
        return_to,
    };
    app.set_info(format!("Restored version {version_number} as a new version"));
    Ok(())
}

/// Opens the title popup for the selected entry, pre-filled with its
/// current title so `t` reads as "show me the title and let me edit it".
fn open_title_prompt(app: &mut App) {
    let Some(entry) = app.store.entries.get(app.selected) else {
        app.set_error("No entry selected");
        return;
    };
    app.mode = Mode::TitlePrompt {
        entry_id: entry.id.clone(),
        input: entry.title().unwrap_or_default().to_string(),
        return_to: Box::new(Mode::Normal),
    };
}

/// Retitling a saved entry is a content change like any other, so it goes
/// through `push_version` — the old title stays readable in `:history`.
fn submit_title_prompt(app: &mut App) {
    let Mode::TitlePrompt {
        entry_id,
        input,
        return_to,
    } = app.mode.clone()
    else {
        return;
    };
    app.mode = *return_to;

    let trimmed = input.trim();
    let new_title = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };

    let Some(entry) = app.store.find(&entry_id) else {
        app.set_error(format!("No entry with ID {entry_id}"));
        return;
    };
    if entry.current().title == new_title {
        app.set_info("Title unchanged");
        return;
    }
    let current = entry.current().clone();
    if let Some(entry) = app.store.find_mut(&entry_id) {
        entry.push_version(new_title.clone(), current.text, current.formatting);
    }
    app.save_store();
    match new_title {
        Some(t) => app.set_info(format!("Titled \"{t}\" (saved as a new version)")),
        None => app.set_info("Title removed (saved as a new version)"),
    }
}

/// Sets (or with an empty `title`, clears) the draft's title. Like
/// formatting, this only touches the draft — `Ctrl+S` is what writes it
/// into a new version, so an old version keeps the title it had.
pub fn set_title(app: &mut App, title: &str) -> Result<(), String> {
    if !matches!(app.mode, Mode::Editor) {
        return Err("`:title` only works while editing an entry".into());
    }
    let title = title.trim();
    if title.is_empty() {
        app.editor.title = None;
        app.set_info("Title cleared — Ctrl+S to save");
    } else {
        app.editor.title = Some(title.to_string());
        app.set_info(format!("Title set to \"{title}\" — Ctrl+S to save"));
    }
    Ok(())
}

/// `:done` / `:undone` — crosses the target text out, or un-crosses it.
///
/// Works on the selection when there is one, otherwise on the cursor's
/// line, so the common case (cursor on a todo line, `:done`) needs no
/// selecting. Unlike `toggle_format`, applying it twice is harmless: the
/// range logic in `Formatting::set`/`clear` merges rather than stacks.
pub fn set_done(app: &mut App, done: bool) -> Result<(), String> {
    if !matches!(app.mode, Mode::Editor) {
        return Err("`:done` only works while editing an entry".into());
    }
    let (start, end) = match app.editor.selection_range() {
        Some((s, e)) if s != e => (s, e),
        _ => text::line_bounds(&app.editor.text, app.editor.cursor),
    };
    if start == end {
        return Err("Nothing on this line to mark".into());
    }
    let already = app
        .editor
        .formatting
        .covers(start, end, MarkKind::Strikethrough);
    if done {
        app.editor.formatting.set(start, end, MarkKind::Strikethrough);
        if already {
            app.set_info("Already done");
        } else {
            app.set_info("Marked done — Ctrl+S to save");
        }
    } else {
        app.editor
            .formatting
            .clear(start, end, MarkKind::Strikethrough);
        app.set_info("Marked not done — Ctrl+S to save");
    }
    Ok(())
}

/// Toggles a formatting mark over the current editor selection. If an
/// identical mark already covers exactly that range, it's removed
/// instead (a second `:i` on the same selection un-italicizes it).
pub fn toggle_format(app: &mut App, kind: MarkKind) -> Result<(), String> {
    // `app.editor` outlives the editor view, so without this guard a
    // format command run from the journal or history view would report
    // success while marking a buffer nothing will ever save.
    if !matches!(app.mode, Mode::Editor) {
        return Err("Formatting only works while editing an entry".into());
    }
    let Some((start, end)) = app.editor.selection_range() else {
        return Err("Select text first (press v), then run the format command".into());
    };
    if start == end {
        return Err("Selection is empty".into());
    }
    let marks = &mut app.editor.formatting.marks;
    if let Some(pos) = marks
        .iter()
        .position(|m| m.kind == kind && m.start == start && m.end == end)
    {
        marks.remove(pos);
        app.set_info(format!(
            "Removed {}",
            crate::format::find_by_kind(kind).name
        ));
    } else {
        marks.push(Mark { start, end, kind });
        app.set_info(format!("Applied {}", crate::format::find_by_kind(kind).name));
    }
    Ok(())
}
