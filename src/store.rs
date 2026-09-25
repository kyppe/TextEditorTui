//! Persistence: the whole journal lives in a single JSON file.
//!
//! To add a new persistent field, see FEATURES.md "Add a new persistent
//! data field" — in short: add it to a struct in `entry.rs` with
//! `#[serde(default)]` so existing files on disk keep loading.

use crate::entry::{Entry, EntryId};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Store {
    pub entries: Vec<Entry>,
}

impl Store {
    /// `~/.local/share/textpoppup/entries.json` on Linux (via the `dirs`
    /// crate's platform-appropriate data directory on other OSes).
    pub fn path() -> PathBuf {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("textpoppup");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("entries.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let mut store: Store = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Store::default(),
        };
        // Everything downstream relies on "an entry always has at least one
        // version" (`Entry::current`, the history view's `len() - 1`). A
        // hand-edited or half-written file could break that and panic mid
        // render, so enforce it once here at the boundary instead.
        store.entries.retain(|e| !e.versions.is_empty());
        store
    }

    /// Writes via a temp file + rename so a crash mid-write can't corrupt
    /// or truncate the journal.
    pub fn save(&self) -> io::Result<()> {
        let path = Self::path();
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(self).expect("Store always serializes");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    /// A fresh ID guaranteed not to collide with an existing entry.
    pub fn unique_id(&self) -> EntryId {
        loop {
            let id = crate::id::generate();
            if self.find(&id).is_none() {
                return id;
            }
        }
    }
}
