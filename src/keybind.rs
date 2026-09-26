//! The one place every keyboard shortcut is defined. `resolve` maps a
//! `(ModeKind, KeyEvent)` to an `Action`; `action::apply` is the only
//! thing that acts on an `Action`. To add or change a shortcut, edit
//! `resolve` below — see FEATURES.md "Add a new keyboard shortcut".

use crate::app::ModeKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub enum Action {
    NoOp,
    RequestQuit,
    /// First key of a chord (`g` in `gg`) — stored on `App::pending_key`
    /// and fed back into `resolve` on the next key press.
    SetPending(char),

    // --- Normal mode: browsing entries ---
    SelectUp,
    SelectDown,
    SelectFirst,
    SelectLast,
    NewEntry,
    EditSelected,
    DeleteSelected,
    OpenHistorySelected,
    OpenTitlePrompt,
    EnterCommand,
    ShowHelp,

    // --- Editor mode, normal (typing = false) ---
    EditorEnterTyping,
    EditorAppendTyping,
    EditorMoveLeft,
    EditorMoveRight,
    EditorMoveUp,
    EditorMoveDown,
    EditorLineStart,
    EditorLineEnd,
    EditorToggleSelection,
    EditorSelectLine,
    EditorSaveExit,
    EditorCancelExit,
    EditorOpenHistory,
    // Vim motions
    EditorBufferStart,
    EditorBufferEnd,
    EditorWordForward,
    EditorWordBack,
    EditorWordEnd,
    EditorFirstNonBlank,
    // Vim edits
    EditorDeleteChar,
    EditorDeleteLine,
    EditorDeleteToLineEnd,
    EditorDeleteToLineStart,
    EditorDeleteWord,
    EditorDeleteWordBack,
    EditorChangeWord,
    EditorChangeLine,
    EditorChangeToLineEnd,
    EditorJoinLines,
    EditorReplaceChar(char),
    EditorIndent,
    EditorDedent,
    EditorUndo,
    EditorRedo,
    // Yank / paste
    EditorYankLine,
    EditorYankSelection,
    EditorPasteAfter,
    EditorPasteBefore,
    EditorDeleteSelection,
    EditorChangeSelection,
    EditorPasteOverSelection,
    EditorClearSelection,
    EditorOpenLineBelow,
    EditorOpenLineAbove,
    EditorInsertAtLineStart,
    EditorAppendAtLineEnd,

    // --- Editor mode, typing = true ---
    EditorInsertChar(char),
    EditorBackspace,
    EditorNewline,
    EditorExitTyping,

    // --- Command line ---
    CommandInsertChar(char),
    CommandBackspace,
    CommandSubmit,
    CommandCancel,

    // --- History view ---
    HistoryPrev,
    HistoryNext,
    HistoryFirst,
    HistoryLast,
    HistoryGoto(usize),
    HistoryClose,
    HistoryEnterCommand,

    // --- Title popup ---
    TitleInsertChar(char),
    TitleBackspace,
    TitleSubmit,
    TitleCancel,

    // --- Confirm popup ---
    ConfirmYes,
    ConfirmNo,

    // --- Help popup ---
    HelpScrollDown,
    HelpScrollUp,
    HelpClose,
}

/// `pending` is the first key of an in-progress chord (see
/// `Action::SetPending`), so `gg` and `dd` resolve here rather than being
/// special-cased elsewhere.
pub fn resolve(mode: ModeKind, key: KeyEvent, pending: Option<char>) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Chord completions first: they outrank the single-key meaning of the
    // same key (`d` alone deletes an entry, `dd` deletes a line).
    if let Some(first) = pending {
        use ModeKind::{EditorNormal, EditorVisual, History, Normal};
        return match (mode, first, key.code) {
            (Normal, 'g', KeyCode::Char('g')) => Action::SelectFirst,
            (History, 'g', KeyCode::Char('g')) => Action::HistoryFirst,
            (EditorNormal | EditorVisual, 'g', KeyCode::Char('g')) => Action::EditorBufferStart,
            // Operators: `d`/`c` + a motion.
            (EditorNormal, 'd', KeyCode::Char('d')) => Action::EditorDeleteLine,
            (EditorNormal, 'd', KeyCode::Char('w')) => Action::EditorDeleteWord,
            (EditorNormal, 'd', KeyCode::Char('b')) => Action::EditorDeleteWordBack,
            (EditorNormal, 'd', KeyCode::Char('$')) => Action::EditorDeleteToLineEnd,
            (EditorNormal, 'd', KeyCode::Char('0')) => Action::EditorDeleteToLineStart,
            (EditorNormal, 'c', KeyCode::Char('c')) => Action::EditorChangeLine,
            (EditorNormal, 'c', KeyCode::Char('w')) => Action::EditorChangeWord,
            (EditorNormal, 'c', KeyCode::Char('$')) => Action::EditorChangeToLineEnd,
            (EditorNormal, 'y', KeyCode::Char('y')) => Action::EditorYankLine,
            // `r` takes whatever character comes next.
            (EditorNormal, 'r', KeyCode::Char(c)) => Action::EditorReplaceChar(c),
            // Anything else abandons the chord and is handled afresh.
            _ => resolve(mode, key, None),
        };
    }

    match mode {
        ModeKind::Normal => match key.code {
            KeyCode::Char('q') => Action::RequestQuit,
            KeyCode::Char('j') | KeyCode::Down => Action::SelectDown,
            KeyCode::Char('k') | KeyCode::Up => Action::SelectUp,
            KeyCode::Char('g') => Action::SetPending('g'),
            KeyCode::Char('G') => Action::SelectLast,
            KeyCode::Char('n') | KeyCode::Char('o') => Action::NewEntry,
            KeyCode::Char('i') | KeyCode::Enter => Action::EditSelected,
            KeyCode::Char('d') => Action::DeleteSelected,
            KeyCode::Char('t') => Action::OpenTitlePrompt,
            KeyCode::Char('H') => Action::OpenHistorySelected,
            KeyCode::Char('?') => Action::ShowHelp,
            KeyCode::Char(':') => Action::EnterCommand,
            _ => Action::NoOp,
        },

        ModeKind::EditorNormal => match key.code {
            KeyCode::Char('i') => Action::EditorEnterTyping,
            KeyCode::Char('a') => Action::EditorAppendTyping,
            KeyCode::Char('I') => Action::EditorInsertAtLineStart,
            KeyCode::Char('A') => Action::EditorAppendAtLineEnd,
            KeyCode::Char('o') => Action::EditorOpenLineBelow,
            KeyCode::Char('O') => Action::EditorOpenLineAbove,
            KeyCode::Char('h') | KeyCode::Left => Action::EditorMoveLeft,
            KeyCode::Char('l') | KeyCode::Right => Action::EditorMoveRight,
            KeyCode::Char('k') | KeyCode::Up => Action::EditorMoveUp,
            KeyCode::Char('j') | KeyCode::Down => Action::EditorMoveDown,
            KeyCode::Char('w') => Action::EditorWordForward,
            KeyCode::Char('b') => Action::EditorWordBack,
            KeyCode::Char('e') => Action::EditorWordEnd,
            KeyCode::Char('g') => Action::SetPending('g'),
            KeyCode::Char('G') => Action::EditorBufferEnd,
            KeyCode::Char('^') => Action::EditorFirstNonBlank,
            KeyCode::Char('0') | KeyCode::Home => Action::EditorLineStart,
            KeyCode::Char('$') | KeyCode::End => Action::EditorLineEnd,
            KeyCode::Char('x') => Action::EditorDeleteChar,
            KeyCode::Char('d') => Action::SetPending('d'),
            KeyCode::Char('c') => Action::SetPending('c'),
            KeyCode::Char('y') => Action::SetPending('y'),
            // Guarded arm must precede the bare `r`, or Ctrl+r never matches.
            KeyCode::Char('r') if ctrl => Action::EditorRedo,
            KeyCode::Char('r') => Action::SetPending('r'),
            KeyCode::Char('D') => Action::EditorDeleteToLineEnd,
            KeyCode::Char('C') => Action::EditorChangeToLineEnd,
            KeyCode::Char('Y') => Action::EditorYankLine,
            KeyCode::Char('p') => Action::EditorPasteAfter,
            KeyCode::Char('P') => Action::EditorPasteBefore,
            KeyCode::Char('J') => Action::EditorJoinLines,
            KeyCode::Char('u') => Action::EditorUndo,
            KeyCode::Char('v') => Action::EditorToggleSelection,
            KeyCode::Char('V') => Action::EditorSelectLine,
            KeyCode::Char('H') => Action::EditorOpenHistory,
            KeyCode::Char(':') => Action::EnterCommand,
            KeyCode::Esc => Action::EditorCancelExit,
            KeyCode::Char('s') if ctrl => Action::EditorSaveExit,
            _ => Action::NoOp,
        },

        // Visual mode: motions come from the normal-mode table (see the
        // fall-through), and these keys act on the selection instead.
        ModeKind::EditorVisual => match key.code {
            KeyCode::Char('y') => Action::EditorYankSelection,
            KeyCode::Char('d') | KeyCode::Char('x') => Action::EditorDeleteSelection,
            KeyCode::Char('c') => Action::EditorChangeSelection,
            KeyCode::Char('p') => Action::EditorPasteOverSelection,
            KeyCode::Char('v') => Action::EditorToggleSelection,
            KeyCode::Char('V') => Action::EditorSelectLine,
            KeyCode::Esc => Action::EditorClearSelection,
            KeyCode::Char(':') => Action::EnterCommand,
            KeyCode::Char('s') if ctrl => Action::EditorSaveExit,
            _ => resolve(ModeKind::EditorNormal, key, None),
        },

        ModeKind::EditorTyping => match key.code {
            KeyCode::Esc => Action::EditorExitTyping,
            KeyCode::Enter => Action::EditorNewline,
            KeyCode::Backspace => Action::EditorBackspace,
            KeyCode::Tab => Action::EditorIndent,
            KeyCode::BackTab => Action::EditorDedent,
            KeyCode::Left => Action::EditorMoveLeft,
            KeyCode::Right => Action::EditorMoveRight,
            KeyCode::Up => Action::EditorMoveUp,
            KeyCode::Down => Action::EditorMoveDown,
            KeyCode::Char('s') if ctrl => Action::EditorSaveExit,
            KeyCode::Char('v') if ctrl => Action::EditorPasteBefore,
            KeyCode::Char(c) => Action::EditorInsertChar(c),
            _ => Action::NoOp,
        },

        ModeKind::Command => match key.code {
            KeyCode::Enter => Action::CommandSubmit,
            KeyCode::Esc => Action::CommandCancel,
            KeyCode::Backspace => Action::CommandBackspace,
            KeyCode::Char(c) => Action::CommandInsertChar(c),
            _ => Action::NoOp,
        },

        ModeKind::History => match key.code {
            KeyCode::Char(c @ '1'..='9') if ctrl => {
                Action::HistoryGoto(c.to_digit(10).unwrap() as usize - 1)
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => Action::HistoryPrev,
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => Action::HistoryNext,
            KeyCode::Char('g') => Action::SetPending('g'),
            KeyCode::Char('G') => Action::HistoryLast,
            KeyCode::Char(':') => Action::HistoryEnterCommand,
            KeyCode::Esc | KeyCode::Char('q') => Action::HistoryClose,
            _ => Action::NoOp,
        },

        ModeKind::TitlePrompt => match key.code {
            KeyCode::Enter => Action::TitleSubmit,
            KeyCode::Esc => Action::TitleCancel,
            KeyCode::Backspace => Action::TitleBackspace,
            KeyCode::Char(c) => Action::TitleInsertChar(c),
            _ => Action::NoOp,
        },

        ModeKind::Confirm => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Action::ConfirmYes,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Action::ConfirmNo,
            _ => Action::NoOp,
        },

        ModeKind::Help => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Action::HelpScrollDown,
            KeyCode::Char('k') | KeyCode::Up => Action::HelpScrollUp,
            _ => Action::HelpClose,
        },
    }
}
