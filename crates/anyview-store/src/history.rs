//! The recently-viewed history: types and the pure step that adds a view to it.

use crate::label::ResumeLabel;
use crate::viewed::Viewed;
use anyview_core::{FilePath, FormatKind};
use std::num::NonZeroUsize;

/// How many files the history keeps. At least one: a history that holds nothing is switched off
/// by not recording, not by a zero. The caller reads it from settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HistoryCap(NonZeroUsize);

impl HistoryCap {
    /// 200 files: a few months of ordinary use, and a file (about 40 KB) small enough that the
    /// launcher can read it each time it opens without noticing.
    pub const DEFAULT: HistoryCap = HistoryCap(NonZeroUsize::MIN.saturating_add(199));

    /// A cap of `count` files, or `None` for zero.
    pub fn new(count: usize) -> Option<HistoryCap> {
        NonZeroUsize::new(count).map(HistoryCap)
    }

    /// The number of files kept.
    pub fn get(self) -> usize {
        self.0.get()
    }
}

impl Default for HistoryCap {
    fn default() -> HistoryCap {
        HistoryCap::DEFAULT
    }
}

/// One viewed file, as the launcher needs it: which file, what kind, when, and where it was left.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    /// The file.
    pub path: FilePath,
    /// What it was when viewed.
    pub kind: FormatKind,
    /// When it was last viewed.
    pub viewed: Viewed,
    /// Where it was left, for a row's subtitle.
    pub label: ResumeLabel,
}

/// The whole of `history.json`: files by recency, the most recently viewed first, each path once.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct History {
    /// Newest first.
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
}

/// `history` after viewing `entry`: any earlier entry for the same path is dropped, `entry` goes
/// first, and only the newest `cap` entries stay. The order is the order of recording, so a
/// clock that stepped back does not reorder the list.
#[must_use]
pub fn history_after_view(history: &History, entry: HistoryEntry, cap: HistoryCap) -> History {
    let entries = std::iter::once(entry.clone())
        .chain(
            history
                .entries
                .iter()
                .filter(|e| e.path != entry.path)
                .cloned(),
        )
        .take(cap.get())
        .collect();
    History { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, viewed: u64, page: u32) -> HistoryEntry {
        HistoryEntry {
            path: FilePath::new(path).unwrap(),
            kind: FormatKind::Pdf,
            viewed: Viewed(viewed),
            label: ResumeLabel::Page { number: page },
        }
    }

    fn history(entries: &[(&str, u64, u32)]) -> History {
        History {
            entries: entries.iter().map(|(p, v, n)| entry(p, *v, *n)).collect(),
        }
    }

    #[test]
    fn viewing_a_file_moves_it_to_the_front_and_caps_the_list() {
        // (name, before, viewed, cap, after)
        type Row = (
            &'static str,
            &'static [(&'static str, u64, u32)],
            (&'static str, u64, u32),
            usize,
            &'static [(&'static str, u64, u32)],
        );
        const CASES: &[Row] = &[
            ("into empty", &[], ("/a", 1, 1), 3, &[("/a", 1, 1)]),
            (
                "new file goes first",
                &[("/a", 1, 1)],
                ("/b", 2, 1),
                3,
                &[("/b", 2, 1), ("/a", 1, 1)],
            ),
            (
                "seen again moves to the front with the new label and time",
                &[("/b", 2, 1), ("/a", 1, 1)],
                ("/a", 3, 9),
                3,
                &[("/a", 3, 9), ("/b", 2, 1)],
            ),
            (
                "seen again from the middle leaves the rest in order",
                &[("/c", 3, 1), ("/b", 2, 1), ("/a", 1, 1)],
                ("/b", 4, 2),
                3,
                &[("/b", 4, 2), ("/c", 3, 1), ("/a", 1, 1)],
            ),
            (
                "the oldest falls off at the cap",
                &[("/c", 3, 1), ("/b", 2, 1), ("/a", 1, 1)],
                ("/d", 4, 1),
                3,
                &[("/d", 4, 1), ("/c", 3, 1), ("/b", 2, 1)],
            ),
            (
                "a repeat at the cap drops nothing",
                &[("/c", 3, 1), ("/b", 2, 1), ("/a", 1, 1)],
                ("/a", 4, 1),
                3,
                &[("/a", 4, 1), ("/c", 3, 1), ("/b", 2, 1)],
            ),
            (
                "a lowered cap trims an over-long list",
                &[("/c", 3, 1), ("/b", 2, 1), ("/a", 1, 1)],
                ("/d", 4, 1),
                2,
                &[("/d", 4, 1), ("/c", 3, 1)],
            ),
            (
                "an older time still goes first",
                &[("/a", 10, 1)],
                ("/b", 5, 1),
                3,
                &[("/b", 5, 1), ("/a", 10, 1)],
            ),
            (
                "a path is the same after normalising",
                &[("/a/b", 1, 1)],
                ("/a/./c/../b", 2, 1),
                3,
                &[("/a/b", 2, 1)],
            ),
        ];
        for (name, before, viewed, cap, after) in CASES {
            let got = history_after_view(
                &history(before),
                entry(viewed.0, viewed.1, viewed.2),
                HistoryCap::new(*cap).unwrap(),
            );
            assert_eq!(got, history(after), "{name}");
        }
    }

    #[test]
    fn a_cap_of_zero_does_not_exist() {
        assert_eq!(HistoryCap::new(0), None);
        assert_eq!(HistoryCap::new(5).map(HistoryCap::get), Some(5));
        assert_eq!(HistoryCap::default().get(), 200);
    }

    #[test]
    fn history_round_trips_in_its_stored_form() {
        let stored = history(&[("/a.pdf", 7, 3)]);
        let json = r#"{"entries":[{"path":"/a.pdf","kind":"pdf","viewed":7,"label":{"kind":"page","v":{"number":3}}}]}"#;
        assert_eq!(serde_json::to_string(&stored).unwrap(), json);
        assert_eq!(serde_json::from_str::<History>(json).unwrap(), stored);
    }

    #[test]
    fn unknown_keys_do_not_stop_a_load_and_a_missing_list_is_empty() {
        let json = r#"{"entries":[{"path":"/a.pdf","kind":"pdf","viewed":7,"label":{"kind":"unlabelled"},"extra":1}],"other":2}"#;
        assert_eq!(
            serde_json::from_str::<History>(json).unwrap().entries.len(),
            1
        );
        assert_eq!(
            serde_json::from_str::<History>("{}").unwrap(),
            History::default()
        );
    }
}
