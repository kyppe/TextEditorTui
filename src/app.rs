//! Central application state. `Mode` is the one enum that determines
//! both what's on screen (`ui/mod.rs`) and which keys mean what
//! (`keybind.rs`) — see ARCHITECTURE.md for how those three connect.

use crate::entry::{EntryId, Formatting};
use crate::store::Store;

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteEntry(EntryId),
}

#[derive(Debug, Clone)]
pub enum Mode {
    /// Browsing the list of entries.
    Normal,
    /// Composing or editing one entry's text.
    Editor,
    /// The `:` command line is open; `return_to` is where Enter/Esc goes.
    Command { return_to: Box<Mode> },
    /// Viewing the version-tab history of one entry (read-only).
    /// `return_to` is where Esc goes back to, so consulting history while
    /// composing an entry doesn't throw the unsaved draft away.
    History {
        entry_id: EntryId,
        selected: usize,
        return_to: Box<Mode>,
    },
    /// A yes/no confirmation popup.
    Confirm {
        message: String,
        action: ConfirmAction,
        return_to: Box<Mode>,
    },
    /// The keybinding/command cheat sheet, scrolled by `scroll` rows.
    Help { return_to: Box<Mode>, scroll: u16 },
    /// Small popup for naming/renaming an entry from the journal view,
    /// pre-filled with the entry's current title if it has one.
    TitlePrompt {
        entry_id: EntryId,
        input: String,
        return_to: Box<Mode>,
    },
}

/// A rough classification of `Mode` used by the keybinding table, so
/// adding a new `Mode` variant with behavior identical to an existing
/// one doesn't require touching `keybind.rs` at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeKind {
    Normal,
    EditorNormal,
    EditorVisual,
    EditorTyping,
    Command,
    History,
    Confirm,
    Help,
    TitlePrompt,
}

pub enum StatusLevel {
    Info,
    Error,
}

/// Everything `u` needs to put back. Whole-draft snapshots rather than a
/// diff/delta log: drafts are short, and this can't drift out of sync with
/// the formatting marks the way replaying edits could.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub title: Option<String>,
    pub text: String,
    pub cursor: usize,
    pub formatting: Formatting,
}

/// Cap on remembered undo steps, so a long session can't grow unbounded.
const UNDO_LIMIT: usize = 500;

pub struct EditorState {
    /// `None` while composing a brand-new entry that hasn't been saved yet.
    pub entry_id: Option<EntryId>,
    /// Draft title, set with `:title`; saved into the next version.
    pub title: Option<String>,
    pub text: String,
    /// Character offset, not byte offset (see `text.rs`).
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
    /// `V` makes the selection *linewise*: extending it with j/k covers
    /// whole lines, as in Vim, rather than stopping mid-line wherever the
    /// cursor happens to land. See `selection_range`.
    pub selection_linewise: bool,
    pub formatting: Formatting,
    /// Vim-style: true while characters typed are inserted as text.
    /// Starts `false` — the editor opens in NORMAL mode, like Vim.
    pub typing: bool,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
}

impl EditorState {
    pub fn empty() -> Self {
        EditorState {
            entry_id: None,
            title: None,
            text: String::new(),
            cursor: 0,
            selection_anchor: None,
            selection_linewise: false,
            formatting: Formatting::default(),
            typing: false,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            title: self.title.clone(),
            text: self.text.clone(),
            cursor: self.cursor,
            formatting: self.formatting.clone(),
        }
    }

    pub fn restore(&mut self, s: Snapshot) {
        self.title = s.title;
        self.cursor = s.cursor.min(crate::text::char_len(&s.text));
        self.text = s.text;
        self.formatting = s.formatting;
        self.selection_anchor = None;
        self.selection_linewise = false;
    }

    /// Records the pre-edit state for `u`, and drops the redo branch —
    /// a fresh edit after undoing makes the undone future unreachable,
    /// same as Vim.
    pub fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The selected character range. A linewise selection (`V`) is widened
    /// to the full first and last lines it touches, so moving the cursor up
    /// or down keeps whole lines selected instead of cutting one in half.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let (lo, hi) = if anchor <= self.cursor {
            (anchor, self.cursor)
        } else {
            (self.cursor, anchor)
        };
        if self.selection_linewise {
            let (start, _) = crate::text::line_bounds(&self.text, lo);
            let (_, end) = crate::text::line_bounds(&self.text, hi);
            Some((start, end))
        } else {
            Some((lo, hi))
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_linewise = false;
    }
}

pub struct App {
    pub store: Store,
    pub mode: Mode,
    /// Index into `store.entries` for the Normal-mode list.
    pub selected: usize,
    pub editor: EditorState,
    pub command_input: String,
    pub status: Option<(String, StatusLevel)>,
    pub should_quit: bool,
    /// Column ranges of the last-rendered history tabs (start, end,
    /// version index), for mouse hit-testing. Recomputed every frame in
    /// `ui::history`; see that module.
    pub history_tab_cols: Vec<(u16, u16, usize)>,
    /// Terminal row the history tabs were last drawn on.
    pub history_tab_row: u16,
    /// Largest useful help-popup scroll offset at the current terminal
    /// size; set by `ui::popup::help` each frame it draws.
    pub help_max_scroll: u16,
    /// First key of a pending multi-key motion (`g` of `gg`, `d` of `dd`).
    /// Consulted by `keybind::resolve` and cleared by `action::apply`.
    pub pending_key: Option<char>,
    /// The yank register, mirrored to the system clipboard.
    pub register: crate::clipboard::Register,
}

impl App {
    pub fn new() -> Self {
        let store = Store::load();
        let selected = store.entries.len().saturating_sub(1);
        App {
            store,
            mode: Mode::Normal,
            selected,
            editor: EditorState::empty(),
            command_input: String::new(),
            status: None,
            should_quit: false,
            history_tab_cols: Vec::new(),
            history_tab_row: 0,
            help_max_scroll: 0,
            pending_key: None,
            register: crate::clipboard::Register::default(),
        }
    }

    pub fn mode_kind(&self) -> ModeKind {
        match &self.mode {
            Mode::Normal => ModeKind::Normal,
            Mode::Editor => {
                if self.editor.typing {
                    ModeKind::EditorTyping
                } else if self.editor.selection_anchor.is_some() {
                    ModeKind::EditorVisual
                } else {
                    ModeKind::EditorNormal
                }
            }
            Mode::Command { .. } => ModeKind::Command,
            Mode::History { .. } => ModeKind::History,
            Mode::Confirm { .. } => ModeKind::Confirm,
            Mode::Help { .. } => ModeKind::Help,
            Mode::TitlePrompt { .. } => ModeKind::TitlePrompt,
        }
    }

    pub fn set_info(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), StatusLevel::Info));
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), StatusLevel::Error));
    }

    pub fn save_store(&mut self) {
        if let Err(e) = self.store.save() {
            self.set_error(format!("Failed to save: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_with(text: &str, cursor: usize) -> EditorState {
        let mut e = EditorState::empty();
        e.text = text.to_string();
        e.cursor = cursor;
        e
    }

    /// `V` then moving up must keep *both* lines fully selected. Before
    /// linewise selections existed, the range collapsed onto the newline
    /// between them and the highlight vanished entirely.
    #[test]
    fn linewise_selection_covers_whole_lines_in_both_directions() {
        let text = "line one\nline two\nline three";
        //          0..8       9..17      18..28
        let mut e = editor_with(text, 18); // start of "line three"
        e.selection_anchor = Some(18);
        e.selection_linewise = true;
        assert_eq!(e.selection_range(), Some((18, 28)));

        // Cursor up into the previous line: both lines stay whole.
        e.cursor = 12;
        assert_eq!(e.selection_range(), Some((9, 28)));

        // And up again: three whole lines.
        e.cursor = 3;
        assert_eq!(e.selection_range(), Some((0, 28)));
    }

    #[test]
    fn charwise_selection_is_left_exactly_where_the_cursor_is() {
        let mut e = editor_with("line one\nline two", 12);
        e.selection_anchor = Some(4);
        assert_eq!(e.selection_range(), Some((4, 12)));
    }

    #[test]
    fn clearing_a_selection_also_clears_linewise() {
        let mut e = editor_with("a\nb", 0);
        e.selection_anchor = Some(0);
        e.selection_linewise = true;
        e.clear_selection();
        // A later charwise `v` must not inherit linewise behaviour.
        e.cursor = 2;
        e.selection_anchor = Some(2);
        assert_eq!(e.selection_range(), Some((2, 2)));
    }
}
