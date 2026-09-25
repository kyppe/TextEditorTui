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

pub struct EditorState {
    /// `None` while composing a brand-new entry that hasn't been saved yet.
    pub entry_id: Option<EntryId>,
    /// Draft title, set with `:title`; saved into the next version.
    pub title: Option<String>,
    pub text: String,
    /// Character offset, not byte offset (see `text.rs`).
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
    pub formatting: Formatting,
    /// Vim-style: true while characters typed are inserted as text.
    pub typing: bool,
}

impl EditorState {
    pub fn empty() -> Self {
        EditorState {
            entry_id: None,
            title: None,
            text: String::new(),
            cursor: 0,
            selection_anchor: None,
            formatting: Formatting::default(),
            typing: true,
        }
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            if anchor <= self.cursor {
                (anchor, self.cursor)
            } else {
                (self.cursor, anchor)
            }
        })
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
        }
    }

    pub fn mode_kind(&self) -> ModeKind {
        match &self.mode {
            Mode::Normal => ModeKind::Normal,
            Mode::Editor => {
                if self.editor.typing {
                    ModeKind::EditorTyping
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
