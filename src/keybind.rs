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
    SelectPageUp,
    SelectPageDown,
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
    /// Forward delete from the `Delete` key. Unlike Vim's `x`, plain
    /// keyboard editing keys leave the yank register alone.
    EditorDeleteForward,
    EditorDeleteWordBackPlain,
    EditorPageUp,
    EditorPageDown,
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

    // --- Search (`/`, n, N) ---
    EnterSearch,
    SearchInsertChar(char),
    SearchBackspace,
    SearchSubmit,
    SearchCancel,
    SearchNext,
    SearchPrev,
    /// `gx` — open the link under the cursor.
    OpenLinkUnderCursor,

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

/// The keys a normal keyboard offers for moving and deleting, independent of
/// Vim's letter commands: Home/End ("Orig"/"Fin"), Page Up/Down, Delete, and
/// the Ctrl+arrow word jumps. Shared by all three editor modes so they
/// behave the same whether or not you're typing — `End` has to reach the end
/// of the line in insert mode too, not just in normal mode.
fn standard_editor_key(key: KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    Some(match key.code {
        KeyCode::Home if ctrl => Action::EditorBufferStart,
        KeyCode::End if ctrl => Action::EditorBufferEnd,
        KeyCode::Home => Action::EditorLineStart,
        KeyCode::End => Action::EditorLineEnd,
        KeyCode::PageUp => Action::EditorPageUp,
        KeyCode::PageDown => Action::EditorPageDown,
        KeyCode::Delete => Action::EditorDeleteForward,
        KeyCode::Backspace if ctrl => Action::EditorDeleteWordBackPlain,
        KeyCode::Left if ctrl => Action::EditorWordBack,
        KeyCode::Right if ctrl => Action::EditorWordForward,
        KeyCode::Up if ctrl => Action::EditorBufferStart,
        KeyCode::Down if ctrl => Action::EditorBufferEnd,
        KeyCode::Left => Action::EditorMoveLeft,
        KeyCode::Right => Action::EditorMoveRight,
        KeyCode::Up => Action::EditorMoveUp,
        KeyCode::Down => Action::EditorMoveDown,
        _ => return None,
    })
}

/// `pending` is the first key of an in-progress chord (see
/// `Action::SetPending`), so `gg` and `dd` resolve here rather than being
/// special-cased elsewhere.
pub fn resolve(mode: ModeKind, key: KeyEvent, pending: Option<char>) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // Chord completions first: they outrank the single-key meaning of the
    // same key (`d` alone deletes an entry, `dd` deletes a line).
    if let Some(first) = pending {
        use ModeKind::{EditorNormal, EditorVisual, History, Normal};
        return match (mode, first, key.code) {
            (Normal, 'g', KeyCode::Char('g')) => Action::SelectFirst,
            (History, 'g', KeyCode::Char('g')) => Action::HistoryFirst,
            (EditorNormal | EditorVisual, 'g', KeyCode::Char('g')) => Action::EditorBufferStart,
            // Vim's netrw binding for "open the thing under the cursor".
            (EditorNormal | EditorVisual, 'g', KeyCode::Char('x')) => Action::OpenLinkUnderCursor,
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
            KeyCode::Home => Action::SelectFirst,
            KeyCode::End => Action::SelectLast,
            KeyCode::PageUp => Action::SelectPageUp,
            KeyCode::PageDown => Action::SelectPageDown,
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
            KeyCode::Char('h') => Action::EditorMoveLeft,
            KeyCode::Char('l') => Action::EditorMoveRight,
            KeyCode::Char('k') => Action::EditorMoveUp,
            KeyCode::Char('j') => Action::EditorMoveDown,
            KeyCode::Char('w') => Action::EditorWordForward,
            KeyCode::Char('b') => Action::EditorWordBack,
            KeyCode::Char('e') => Action::EditorWordEnd,
            KeyCode::Char('g') => Action::SetPending('g'),
            KeyCode::Char('G') => Action::EditorBufferEnd,
            KeyCode::Char('^') => Action::EditorFirstNonBlank,
            KeyCode::Char('0') => Action::EditorLineStart,
            KeyCode::Char('$') => Action::EditorLineEnd,
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
            KeyCode::Char('/') => Action::EnterSearch,
            KeyCode::Char('n') => Action::SearchNext,
            KeyCode::Char('N') => Action::SearchPrev,
            KeyCode::Char('v') => Action::EditorToggleSelection,
            KeyCode::Char('V') => Action::EditorSelectLine,
            KeyCode::Char('H') => Action::EditorOpenHistory,
            KeyCode::Char(':') => Action::EnterCommand,
            KeyCode::Esc => Action::EditorCancelExit,
            KeyCode::Char('s') if ctrl => Action::EditorSaveExit,
            // Arrows, Home/End ("Fin"), Page Up/Down, Delete, Ctrl+arrows.
            _ => standard_editor_key(key).unwrap_or(Action::NoOp),
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
            KeyCode::Tab => Action::EditorIndent,
            KeyCode::BackTab => Action::EditorDedent,
            KeyCode::Char('s') if ctrl => Action::EditorSaveExit,
            KeyCode::Char('v') if ctrl => Action::EditorPasteBefore,
            // Ctrl+W is the terminal-wide "delete the word behind me", and
            // Ctrl+Backspace reaches us as either Backspace+CTRL (terminals
            // speaking the Kitty protocol) or as a bare 0x08, which crossterm
            // reports as Ctrl+H — ASCII backspace, so treat it as one.
            KeyCode::Char('w') if ctrl => Action::EditorDeleteWordBackPlain,
            KeyCode::Backspace if ctrl => Action::EditorDeleteWordBackPlain,
            KeyCode::Char('h') if ctrl => Action::EditorBackspace,
            KeyCode::Backspace => Action::EditorBackspace,
            // Arrows, Home/End ("Fin"), Page Up/Down, Delete, Ctrl+arrows.
            _ if standard_editor_key(key).is_some() => {
                standard_editor_key(key).expect("checked just above")
            }
            // Only unmodified characters are text. Without this guard an
            // unbound chord like Ctrl+A would type a literal "a" into the
            // entry instead of doing nothing.
            KeyCode::Char(c) if !ctrl && !alt => Action::EditorInsertChar(c),
            _ => Action::NoOp,
        },

        ModeKind::Command => match key.code {
            KeyCode::Enter => Action::CommandSubmit,
            KeyCode::Esc => Action::CommandCancel,
            KeyCode::Backspace => Action::CommandBackspace,
            // Guarded for the same reason as insert mode: a chord must not
            // arrive as a literal character in the command line.
            KeyCode::Char(c) if !ctrl && !alt => Action::CommandInsertChar(c),
            _ => Action::NoOp,
        },

        ModeKind::Search => match key.code {
            KeyCode::Enter => Action::SearchSubmit,
            KeyCode::Esc => Action::SearchCancel,
            KeyCode::Backspace => Action::SearchBackspace,
            KeyCode::Char(c) if !ctrl && !alt => Action::SearchInsertChar(c),
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
            KeyCode::Home => Action::HistoryFirst,
            KeyCode::End => Action::HistoryLast,
            KeyCode::PageUp => Action::HistoryPrev,
            KeyCode::PageDown => Action::HistoryNext,
            KeyCode::Char(':') => Action::HistoryEnterCommand,
            KeyCode::Esc | KeyCode::Char('q') => Action::HistoryClose,
            _ => Action::NoOp,
        },

        ModeKind::TitlePrompt => match key.code {
            KeyCode::Enter => Action::TitleSubmit,
            KeyCode::Esc => Action::TitleCancel,
            KeyCode::Backspace => Action::TitleBackspace,
            KeyCode::Char(c) if !ctrl && !alt => Action::TitleInsertChar(c),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    /// An unbound chord must do nothing, not arrive as text. Ctrl+A used to
    /// type a literal "a" into the entry.
    #[test]
    fn unbound_chords_are_not_typed_as_characters() {
        for mode in [
            ModeKind::EditorTyping,
            ModeKind::Command,
            ModeKind::TitlePrompt,
        ] {
            for c in ['a', 'z', 'q'] {
                let action = resolve(mode, ctrl_key(KeyCode::Char(c)), None);
                assert!(
                    matches!(action, Action::NoOp),
                    "{mode:?} turned Ctrl+{c} into {action:?}"
                );
            }
        }
        // Plain characters still reach the buffer.
        assert!(matches!(
            resolve(ModeKind::EditorTyping, key(KeyCode::Char('a')), None),
            Action::EditorInsertChar('a')
        ));
    }

    /// "Fin"/End and the other navigation keys have to work while typing,
    /// not just in normal mode.
    #[test]
    fn standard_navigation_keys_work_in_every_editor_mode() {
        for mode in [
            ModeKind::EditorTyping,
            ModeKind::EditorNormal,
            ModeKind::EditorVisual,
        ] {
            assert!(
                matches!(
                    resolve(mode, key(KeyCode::End), None),
                    Action::EditorLineEnd
                ),
                "End did not reach end-of-line in {mode:?}"
            );
            assert!(matches!(
                resolve(mode, key(KeyCode::Home), None),
                Action::EditorLineStart
            ));
            assert!(matches!(
                resolve(mode, key(KeyCode::PageDown), None),
                Action::EditorPageDown
            ));
            assert!(matches!(
                resolve(mode, key(KeyCode::Delete), None),
                Action::EditorDeleteForward
            ));
            assert!(matches!(
                resolve(mode, ctrl_key(KeyCode::Right), None),
                Action::EditorWordForward
            ));
            assert!(matches!(
                resolve(mode, ctrl_key(KeyCode::Home), None),
                Action::EditorBufferStart
            ));
        }
    }

    /// Vim's letter commands must not be shadowed by the shared navigation
    /// table, and `h`/`l` must stay motions rather than becoming text.
    #[test]
    fn vim_letters_still_win_in_normal_mode() {
        assert!(matches!(
            resolve(ModeKind::EditorNormal, key(KeyCode::Char('h')), None),
            Action::EditorMoveLeft
        ));
        assert!(matches!(
            resolve(ModeKind::EditorNormal, key(KeyCode::Char('$')), None),
            Action::EditorLineEnd
        ));
        // Ctrl+r is redo, and must not be swallowed by the `r` chord.
        assert!(matches!(
            resolve(ModeKind::EditorNormal, ctrl_key(KeyCode::Char('r')), None),
            Action::EditorRedo
        ));
        assert!(matches!(
            resolve(ModeKind::EditorNormal, key(KeyCode::Char('r')), None),
            Action::SetPending('r')
        ));
    }

    #[test]
    fn chords_complete_and_abandon_correctly() {
        assert!(matches!(
            resolve(ModeKind::EditorNormal, key(KeyCode::Char('g')), Some('g')),
            Action::EditorBufferStart
        ));
        assert!(matches!(
            resolve(ModeKind::EditorNormal, key(KeyCode::Char('w')), Some('d')),
            Action::EditorDeleteWord
        ));
        // An abandoned chord falls through to the second key's own meaning.
        assert!(matches!(
            resolve(ModeKind::EditorNormal, key(KeyCode::Char('$')), Some('g')),
            Action::EditorLineEnd
        ));
    }
}
