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
        Action::EditorEnterTyping => start_typing(app),
        Action::EditorAppendTyping => {
            app.editor.cursor = (app.editor.cursor + 1).min(text::char_len(&app.editor.text));
            start_typing(app);
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
                app.editor.clear_selection();
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
                cut_range(app, app.editor.cursor, app.editor.cursor + 1, false);
            }
        }
        Action::EditorDeleteToLineEnd => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            cut_range(app, app.editor.cursor, end, false);
        }
        Action::EditorDeleteLine => {
            let (start, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            cut_lines(app, start, end);
        }
        Action::EditorOpenLineBelow => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            start_typing(app);
            app.editor.cursor = end;
            text::insert_char(&mut app.editor.text, end, '\n');
            shift_marks_for_insert(&mut app.editor.formatting, end, 1);
            app.editor.cursor = end + 1;
        }
        Action::EditorOpenLineAbove => {
            let (start, _) = text::line_bounds(&app.editor.text, app.editor.cursor);
            start_typing(app);
            text::insert_char(&mut app.editor.text, start, '\n');
            shift_marks_for_insert(&mut app.editor.formatting, start, 1);
            app.editor.cursor = start;
        }
        Action::EditorInsertAtLineStart => {
            app.editor.cursor = text::first_non_blank(&app.editor.text, app.editor.cursor);
            start_typing(app);
        }
        Action::EditorAppendAtLineEnd => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            app.editor.cursor = end;
            start_typing(app);
        }

        // Vim edits: operators, yank/paste, undo
        Action::EditorDeleteToLineStart => {
            let (start, _) = text::line_bounds(&app.editor.text, app.editor.cursor);
            cut_range(app, start, app.editor.cursor, false);
        }
        Action::EditorDeleteWord => {
            let to = text::next_word_start(&app.editor.text, app.editor.cursor);
            cut_range(app, app.editor.cursor, to, false);
        }
        Action::EditorDeleteWordBack => {
            let to = text::prev_word_start(&app.editor.text, app.editor.cursor);
            cut_range(app, to, app.editor.cursor, false);
        }
        Action::EditorChangeWord => {
            let to = text::next_word_start(&app.editor.text, app.editor.cursor);
            // `cw` stops at the end of the word rather than eating the space
            // that follows, which is what Vim does.
            let end = text::word_end(&app.editor.text, app.editor.cursor);
            let to = to.min(end + 1);
            cut_range(app, app.editor.cursor, to, false);
            start_typing(app);
        }
        Action::EditorChangeToLineEnd => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            cut_range(app, app.editor.cursor, end, false);
            start_typing(app);
        }
        Action::EditorChangeLine => {
            let (start, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            cut_range(app, start, end, false);
            app.editor.cursor = start;
            start_typing(app);
        }
        Action::EditorJoinLines => {
            let (_, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            if end < text::char_len(&app.editor.text) {
                app.editor.push_undo();
                // Replace the newline with a single space, Vim-style.
                text::remove_range(&mut app.editor.text, end, end + 1);
                shift_marks_for_delete(&mut app.editor.formatting, end, 1);
                text::insert_char(&mut app.editor.text, end, ' ');
                shift_marks_for_insert(&mut app.editor.formatting, end, 1);
                app.editor.cursor = end;
            }
        }
        Action::EditorReplaceChar(c) => {
            let len = text::char_len(&app.editor.text);
            if app.editor.cursor < len {
                app.editor.push_undo();
                let at = app.editor.cursor;
                text::remove_range(&mut app.editor.text, at, at + 1);
                text::insert_char(&mut app.editor.text, at, c);
            }
        }
        Action::EditorIndent => {
            // Spaces, not a literal tab: the whole layout/cursor model
            // assumes one character is one column (see text.rs).
            app.editor.push_undo();
            for _ in 0..INDENT {
                text::insert_char(&mut app.editor.text, app.editor.cursor, ' ');
                shift_marks_for_insert(&mut app.editor.formatting, app.editor.cursor, 1);
                app.editor.cursor += 1;
            }
        }
        Action::EditorDedent => dedent(app),
        Action::EditorUndo => {
            if let Some(prev) = app.editor.undo.pop() {
                app.editor.redo.push(app.editor.snapshot());
                app.editor.restore(prev);
                app.set_info("Undo");
            } else {
                app.set_info("Nothing to undo");
            }
        }
        Action::EditorRedo => {
            if let Some(next) = app.editor.redo.pop() {
                app.editor.undo.push(app.editor.snapshot());
                app.editor.restore(next);
                app.set_info("Redo");
            } else {
                app.set_info("Nothing to redo");
            }
        }
        Action::EditorYankLine => {
            let (start, end) = text::line_bounds(&app.editor.text, app.editor.cursor);
            let line = slice(&app.editor.text, start, end);
            crate::clipboard::yank(&mut app.register, line, true);
            app.set_info("Yanked line");
        }
        Action::EditorYankSelection => {
            if let Some((s, e)) = app.editor.selection_range() {
                let linewise = app.editor.selection_linewise;
                let text = slice(&app.editor.text, s, e);
                let n = text.chars().count();
                let lines = text.split('\n').count();
                crate::clipboard::yank(&mut app.register, text, linewise);
                app.editor.clear_selection();
                app.editor.cursor = s;
                if linewise {
                    app.set_info(format!("Yanked {lines} line(s)"));
                } else {
                    app.set_info(format!("Yanked {n} characters"));
                }
            }
        }
        Action::EditorPasteAfter => paste(app, true),
        Action::EditorPasteBefore => paste(app, false),
        Action::EditorDeleteSelection => {
            if let Some((s, e)) = app.editor.selection_range() {
                // A linewise selection takes its lines out whole, newline
                // included, rather than leaving empty lines behind.
                if app.editor.selection_linewise {
                    cut_lines(app, s, e);
                } else {
                    cut_range(app, s, e, false);
                }
            }
        }
        Action::EditorChangeSelection => {
            if let Some((s, e)) = app.editor.selection_range() {
                // Linewise change empties the lines but keeps one to type on
                // (Vim's `S`), so the newline structure around it survives.
                cut_range(app, s, e, false);
                start_typing(app);
            }
        }
        Action::EditorPasteOverSelection => {
            if let Some((s, e)) = app.editor.selection_range() {
                cut_range(app, s, e, false);
                paste(app, false);
            }
        }
        Action::EditorClearSelection => {
            app.editor.clear_selection();
        }

        Action::EditorSelectLine => {
            if app.editor.selection_anchor.is_some() && app.editor.selection_linewise {
                // `V` again leaves visual mode, as in Vim.
                app.editor.clear_selection();
                app.set_info("Selection cleared");
            } else {
                // The anchor stays where the cursor is; `selection_range`
                // widens it to whole lines, which is what keeps j/k
                // extending line-by-line.
                if app.editor.selection_anchor.is_none() {
                    app.editor.selection_anchor = Some(app.editor.cursor);
                }
                app.editor.selection_linewise = true;
                app.set_info("Line selected — j/k extend, :done, or a format command");
            }
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

        // Editor, typing sub-mode. No `push_undo` per keystroke: the
        // snapshot taken when insert mode was entered covers the whole
        // typing session, so `u` undoes what you just typed in one go.
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

/// How many spaces `Tab` inserts in insert mode.
const INDENT: usize = 4;

/// Enters insert mode, snapshotting first so the whole typing session is a
/// single undo step (Vim's behaviour) rather than one step per character.
fn start_typing(app: &mut App) {
    if !app.editor.typing {
        app.editor.push_undo();
        app.editor.typing = true;
    }
}

fn slice(text: &str, start: usize, end: usize) -> String {
    text.chars().skip(start).take(end.saturating_sub(start)).collect()
}

/// Deletes `[start, end)` from the draft, keeping marks and the cursor in
/// step, recording undo, and (unless `keep_register`) yanking the removed
/// text so `d`/`x`/`c` populate the paste register like Vim.
fn cut_range(app: &mut App, start: usize, end: usize, keep_register: bool) {
    if start >= end {
        return;
    }
    app.editor.push_undo();
    if !keep_register {
        let cut = slice(&app.editor.text, start, end);
        crate::clipboard::yank(&mut app.register, cut, false);
    }
    text::remove_range(&mut app.editor.text, start, end);
    shift_marks_for_delete(&mut app.editor.formatting, start, end - start);
    if app.editor.cursor > start {
        app.editor.cursor = start.max(app.editor.cursor.saturating_sub(end - start));
    }
    app.editor.cursor = app.editor.cursor.min(text::char_len(&app.editor.text));
    // A selection anchored into deleted text would now point at the wrong
    // characters, so drop it rather than silently mis-highlighting.
    app.editor.clear_selection();
}

/// Removes the whole lines spanning `[start, end)` — the line content plus
/// the newline that terminates it — yanking them linewise so `p` puts them
/// back as lines. Shared by `dd` and a linewise-visual delete so the two
/// can't drift apart.
fn cut_lines(app: &mut App, start: usize, end: usize) {
    let len = text::char_len(&app.editor.text);
    let lines = slice(&app.editor.text, start, end);
    crate::clipboard::yank(&mut app.register, lines, true);
    // Take the trailing newline with the lines; at the end of the buffer
    // take the preceding one instead, so no blank line is left behind.
    let (from, to) = if end < len {
        (start, end + 1)
    } else if start > 0 {
        (start - 1, end)
    } else {
        (start, end)
    };
    cut_range(app, from, to, true);
    app.editor.cursor = from.min(text::char_len(&app.editor.text));
}

/// Removes up to `INDENT` leading spaces from the cursor's line.
fn dedent(app: &mut App) {
    let (start, _) = text::line_bounds(&app.editor.text, app.editor.cursor);
    let leading = app
        .editor
        .text
        .chars()
        .skip(start)
        .take(INDENT)
        .take_while(|c| *c == ' ')
        .count();
    if leading == 0 {
        return;
    }
    app.editor.push_undo();
    text::remove_range(&mut app.editor.text, start, start + leading);
    shift_marks_for_delete(&mut app.editor.formatting, start, leading);
    app.editor.cursor = app.editor.cursor.saturating_sub(leading).max(start);
}

/// `p` / `P`. Linewise registers open a new line the way Vim does;
/// charwise ones drop in beside the cursor. The register is resolved
/// against the system clipboard first, so text copied elsewhere pastes.
fn paste(app: &mut App, after: bool) {
    let reg = crate::clipboard::resolve_for_paste(&app.register);
    if reg.text.is_empty() {
        app.set_error("Nothing to paste");
        return;
    }
    app.editor.push_undo();
    let n = text::char_len(&reg.text);

    if reg.linewise {
        let (line_start, line_end) = text::line_bounds(&app.editor.text, app.editor.cursor);
        if after {
            let at = line_end;
            text::insert_str(&mut app.editor.text, at, &format!("\n{}", reg.text));
            shift_marks_for_insert(&mut app.editor.formatting, at, n + 1);
            app.editor.cursor = at + 1;
        } else {
            text::insert_str(&mut app.editor.text, line_start, &format!("{}\n", reg.text));
            shift_marks_for_insert(&mut app.editor.formatting, line_start, n + 1);
            app.editor.cursor = line_start;
        }
    } else {
        let (_, line_end) = text::line_bounds(&app.editor.text, app.editor.cursor);
        let at = if after {
            (app.editor.cursor + 1).min(line_end)
        } else {
            app.editor.cursor
        };
        text::insert_str(&mut app.editor.text, at, &reg.text);
        shift_marks_for_insert(&mut app.editor.formatting, at, n);
        // Vim leaves the cursor on the last pasted character.
        app.editor.cursor = at + n.saturating_sub(1);
    }
    app.set_info(if reg.linewise {
        "Pasted line"
    } else {
        "Pasted"
    });
}

fn new_entry(app: &mut App) {
    app.editor = crate::app::EditorState::empty();
    app.mode = Mode::Editor;
    app.set_info("New entry — press i to start typing, Ctrl+S or :wq to save");
}

fn edit_selected(app: &mut App) {
    let Some(entry) = app.store.entries.get(app.selected) else {
        return;
    };
    let current = entry.current();
    // Built from `empty()` rather than a struct literal so adding a field to
    // EditorState doesn't silently need updating in two places.
    let mut editor = crate::app::EditorState::empty();
    editor.entry_id = Some(entry.id.clone());
    editor.title = current.title.clone();
    editor.cursor = text::char_len(&current.text);
    editor.text = current.text.clone();
    editor.formatting = current.formatting.clone();
    app.editor = editor;
    app.mode = Mode::Editor;
}

/// `Ctrl+S` / `:wq` — write the draft (if it changed) and leave the editor.
fn save_editor(app: &mut App) {
    if editor_is_dirty(app) {
        let _ = write_editor(app);
    } else {
        app.set_info("No changes");
    }
    let message = app.status.take();
    leave_editor(app);
    app.status = message;
}

fn cancel_editor(app: &mut App) {
    if app.editor.typing {
        app.editor.typing = false;
        return;
    }
    if app.editor.selection_anchor.is_some() {
        app.editor.clear_selection();
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
    // Running an ex-command drops you back to NORMAL, like Vim: a command
    // issued from VISUAL ends the selection rather than leaving it live.
    if matches!(app.mode, Mode::Editor) {
        app.editor.typing = false;
        app.editor.clear_selection();
    }
}

/// Whether the draft differs from what's stored — what `:q` refuses on and
/// `Ctrl+S` checks before adding a version.
pub fn editor_is_dirty(app: &App) -> bool {
    match &app.editor.entry_id {
        Some(id) => app
            .store
            .find(id)
            .map(|e| {
                let v = e.current();
                v.text != app.editor.text
                    || v.formatting != app.editor.formatting
                    || v.title != app.editor.title
            })
            .unwrap_or(true),
        None => !app.editor.text.trim().is_empty() || app.editor.title.is_some(),
    }
}

/// `:w` — save the draft as a new version and stay in the editor.
pub fn write_editor(app: &mut App) -> Result<(), String> {
    if !matches!(app.mode, Mode::Editor) {
        return Err("`:w` only works while editing an entry (entries are saved already)".into());
    }
    if !editor_is_dirty(app) {
        app.set_info("No changes");
        return Ok(());
    }
    let title = app.editor.title.clone();
    let text = app.editor.text.clone();
    let formatting = app.editor.formatting.clone();

    match app.editor.entry_id.clone() {
        Some(id) => {
            if let Some(entry) = app.store.find_mut(&id) {
                entry.push_version(title, text, formatting);
            }
            app.set_info("Saved new version");
        }
        None => {
            let id = app.store.unique_id();
            let entry = crate::entry::Entry::new(id.clone(), title, text, formatting);
            app.store.entries.push(entry);
            app.selected = app.store.entries.len() - 1;
            // Keep editing the entry that now exists, so a later `:w` adds a
            // version to it instead of creating a second entry.
            app.editor.entry_id = Some(id);
            app.set_info("Entry saved");
        }
    }
    app.save_store();
    Ok(())
}

/// `:q` — leave the editor, or quit the app from anywhere else. Refuses to
/// discard unsaved work unless `force` (`:q!`).
pub fn quit_context(app: &mut App, force: bool) -> Result<(), String> {
    if matches!(app.mode, Mode::Editor) {
        if !force && editor_is_dirty(app) {
            return Err("Unsaved changes — `:wq` to save and close, `:q!` to discard".into());
        }
        leave_editor(app);
        app.set_info("Closed without saving");
        return Ok(());
    }
    app.should_quit = true;
    Ok(())
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
