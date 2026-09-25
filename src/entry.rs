//! Core data model: `Entry`, `Version`, and formatting `Mark`s.
//!
//! History is a first-class concept here, not bolted on: an `Entry` never
//! stores "current text" directly, only a list of immutable `Version`s.
//! Editing always appends a new `Version`; nothing already in `versions`
//! is ever mutated or removed. See ARCHITECTURE.md for the rationale.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

pub type EntryId = String;

/// The set of text-formatting types the app understands.
///
/// To add a new one (e.g. `Bold`), add a variant here and register it in
/// `crate::format::FORMATS` — see FEATURES.md "Add a new formatting type".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkKind {
    Italic,
    Bold,
    Underline,
    Code,
    Highlight,
    /// A run of text acting as a heading inside an entry's body — as
    /// opposed to `Version::title`, which labels the whole entry.
    Heading,
    /// Crossed-out text; what `:done` applies to a finished line.
    Strikethrough,
}

/// A formatting mark applied to a `[start, end)` range of *character*
/// (not byte) offsets within a `Version`'s `text`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub start: usize,
    pub end: usize,
    pub kind: MarkKind,
}

/// All formatting attached to a single version of an entry's text.
/// Kept as its own struct (rather than fields on `Version`) so that a
/// future "formatting mode" or export feature has one obvious type to
/// operate on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Formatting {
    pub marks: Vec<Mark>,
}

impl Formatting {
    /// Applies `kind` across `[start, end)`, absorbing any touching marks
    /// of the same kind into one. Idempotent, so running `:done` twice on
    /// a line doesn't stack duplicate marks on top of each other.
    pub fn set(&mut self, start: usize, end: usize, kind: MarkKind) {
        if start >= end {
            return;
        }
        let (mut lo, mut hi) = (start, end);
        self.marks.retain(|m| {
            // `>=` / `<=` so adjacent runs merge rather than sitting as
            // two marks that would later need clearing separately.
            let touches = m.kind == kind && m.end >= lo && m.start <= hi;
            if touches {
                lo = lo.min(m.start);
                hi = hi.max(m.end);
            }
            !touches
        });
        self.marks.push(Mark {
            start: lo,
            end: hi,
            kind,
        });
    }

    /// Removes `kind` from `[start, end)`, trimming or splitting marks
    /// that extend beyond it so formatting outside the range survives.
    pub fn clear(&mut self, start: usize, end: usize, kind: MarkKind) {
        if start >= end {
            return;
        }
        let mut out = Vec::with_capacity(self.marks.len());
        for m in self.marks.drain(..) {
            if m.kind != kind || m.end <= start || m.start >= end {
                out.push(m);
                continue;
            }
            if m.start < start {
                out.push(Mark {
                    start: m.start,
                    end: start,
                    kind,
                });
            }
            if m.end > end {
                out.push(Mark {
                    start: end,
                    end: m.end,
                    kind,
                });
            }
        }
        out.retain(|m| m.end > m.start);
        self.marks = out;
    }

    /// Whether every character in `[start, end)` carries `kind`.
    pub fn covers(&self, start: usize, end: usize, kind: MarkKind) -> bool {
        start < end
            && self
                .marks
                .iter()
                .any(|m| m.kind == kind && m.start <= start && m.end >= end)
    }
}

/// An immutable snapshot of an entry's content at one point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub version_number: u32,
    pub created_at: DateTime<Local>,
    /// Short label for the entry as of this version. Lives on `Version`
    /// rather than `Entry` so retitling creates a new version and older
    /// versions keep the title they were saved with — the same rule the
    /// text and formatting follow.
    ///
    /// `#[serde(default)]` is what lets journals written before titles
    /// existed keep loading; see FEATURES.md "Add a new persistent field".
    #[serde(default)]
    pub title: Option<String>,
    pub text: String,
    pub formatting: Formatting,
    /// If this version was produced by `:restore`, the version number it
    /// was restored from (kept for display in the history view).
    pub restored_from: Option<u32>,
}

/// A thought/note created during one writing session, identified by a
/// short unique ID and carrying its full edit history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub created_at: DateTime<Local>,
    pub last_edited_at: DateTime<Local>,
    pub versions: Vec<Version>,
}

impl Entry {
    pub fn new(id: EntryId, title: Option<String>, text: String, formatting: Formatting) -> Self {
        let now = Local::now();
        Entry {
            id,
            created_at: now,
            last_edited_at: now,
            versions: vec![Version {
                version_number: 1,
                created_at: now,
                title,
                text,
                formatting,
                restored_from: None,
            }],
        }
    }

    /// The current version's title, if it has one.
    pub fn title(&self) -> Option<&str> {
        self.current().title.as_deref()
    }

    /// The latest (current) version. An entry always has at least one
    /// version, so this never panics on a well-formed entry.
    pub fn current(&self) -> &Version {
        self.versions.last().expect("entry must have >= 1 version")
    }

    /// Number of edits made after creation (i.e. versions beyond the first).
    pub fn edit_count(&self) -> usize {
        self.versions.len().saturating_sub(1)
    }

    pub fn version(&self, number: u32) -> Option<&Version> {
        self.versions.iter().find(|v| v.version_number == number)
    }

    /// Appends a brand-new version. This is the *only* way an entry's
    /// content changes — existing `Version`s are never edited in place.
    pub fn push_version(&mut self, title: Option<String>, text: String, formatting: Formatting) {
        let now = Local::now();
        let number = self.versions.len() as u32 + 1;
        self.versions.push(Version {
            version_number: number,
            created_at: now,
            title,
            text,
            formatting,
            restored_from: None,
        });
        self.last_edited_at = now;
    }

    /// Restoring an old version creates a *new* version with that
    /// version's content; it never truncates or deletes newer versions.
    pub fn restore(&mut self, version_number: u32) -> bool {
        let Some(source) = self.version(version_number).cloned() else {
            return false;
        };
        let now = Local::now();
        let number = self.versions.len() as u32 + 1;
        self.versions.push(Version {
            version_number: number,
            created_at: now,
            title: source.title,
            text: source.text,
            formatting: source.formatting,
            restored_from: Some(version_number),
        });
        self.last_edited_at = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_never_overwrites_history() {
        let mut e = Entry::new("f71d92".into(), None, "Buy milk".into(), Formatting::default());
        e.push_version(None, "Buy milk and bread".into(), Formatting::default());
        e.push_version(None, "Buy milk, bread and eggs".into(), Formatting::default());

        assert_eq!(e.version(1).unwrap().text, "Buy milk");
        assert_eq!(e.version(2).unwrap().text, "Buy milk and bread");
        assert_eq!(e.version(3).unwrap().text, "Buy milk, bread and eggs");
        assert_eq!(e.current().text, "Buy milk, bread and eggs");
        assert_eq!(e.edit_count(), 2);
    }

    #[test]
    fn restore_appends_rather_than_truncates() {
        let mut e = Entry::new("f71d92".into(), None, "v1".into(), Formatting::default());
        e.push_version(None, "v2".into(), Formatting::default());
        e.push_version(None, "v3".into(), Formatting::default());

        assert!(e.restore(1));

        assert_eq!(e.versions.len(), 4);
        assert_eq!(e.current().text, "v1");
        assert_eq!(e.current().restored_from, Some(1));
        // Older versions remain untouched.
        assert_eq!(e.version(1).unwrap().text, "v1");
        assert_eq!(e.version(2).unwrap().text, "v2");
        assert_eq!(e.version(3).unwrap().text, "v3");
    }

    #[test]
    fn restore_unknown_version_fails_without_side_effects() {
        let mut e = Entry::new("id".into(), None, "v1".into(), Formatting::default());
        assert!(!e.restore(99));
        assert_eq!(e.versions.len(), 1);
    }

    /// Titles are per-version, so retitling must not rewrite the title an
    /// older version was saved with.
    #[test]
    fn titles_are_versioned_like_text() {
        let mut e = Entry::new(
            "f71d92".into(),
            Some("Groceries".into()),
            "Buy milk".into(),
            Formatting::default(),
        );
        e.push_version(
            Some("Shopping list".into()),
            "Buy milk and bread".into(),
            Formatting::default(),
        );

        assert_eq!(e.version(1).unwrap().title.as_deref(), Some("Groceries"));
        assert_eq!(
            e.version(2).unwrap().title.as_deref(),
            Some("Shopping list")
        );
        assert_eq!(e.title(), Some("Shopping list"));

        // Restoring v1 brings its title back as a new version, leaving v2's alone.
        assert!(e.restore(1));
        assert_eq!(e.title(), Some("Groceries"));
        assert_eq!(
            e.version(2).unwrap().title.as_deref(),
            Some("Shopping list")
        );
    }

    #[test]
    fn set_is_idempotent_and_merges_adjacent_runs() {
        let mut f = Formatting::default();
        f.set(0, 5, MarkKind::Strikethrough);
        f.set(0, 5, MarkKind::Strikethrough); // repeat must not stack
        assert_eq!(f.marks.len(), 1);
        assert!(f.covers(0, 5, MarkKind::Strikethrough));

        // An adjacent run merges into a single mark.
        f.set(5, 9, MarkKind::Strikethrough);
        assert_eq!(f.marks.len(), 1);
        assert!(f.covers(0, 9, MarkKind::Strikethrough));

        // A different kind is independent.
        f.set(0, 3, MarkKind::Bold);
        assert_eq!(f.marks.len(), 2);
    }

    #[test]
    fn clear_trims_and_splits_without_touching_other_kinds() {
        let mut f = Formatting::default();
        f.set(0, 20, MarkKind::Strikethrough);
        f.set(0, 20, MarkKind::Italic);

        // Clearing the middle splits the strikethrough in two, italic intact.
        f.clear(5, 10, MarkKind::Strikethrough);
        let strikes: Vec<_> = f
            .marks
            .iter()
            .filter(|m| m.kind == MarkKind::Strikethrough)
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(strikes, vec![(0, 5), (10, 20)]);
        assert!(f.covers(0, 20, MarkKind::Italic));

        // Clearing a span that covers everything removes it entirely.
        f.clear(0, 20, MarkKind::Strikethrough);
        assert!(!f.marks.iter().any(|m| m.kind == MarkKind::Strikethrough));
        assert!(f.covers(0, 20, MarkKind::Italic));
    }

    /// Journals written before titles existed must keep loading, with the
    /// title simply absent (this is what `#[serde(default)]` buys us).
    #[test]
    fn entries_saved_without_a_title_still_deserialize() {
        let json = r#"{
            "id": "old001",
            "created_at": "2026-09-25T10:00:00.000000000+01:00",
            "last_edited_at": "2026-09-25T10:00:00.000000000+01:00",
            "versions": [{
                "version_number": 1,
                "created_at": "2026-09-25T10:00:00.000000000+01:00",
                "text": "no title here",
                "formatting": { "marks": [] },
                "restored_from": null
            }]
        }"#;
        let e: Entry = serde_json::from_str(json).expect("pre-title entry must still load");
        assert_eq!(e.title(), None);
        assert_eq!(e.current().text, "no title here");
    }
}
