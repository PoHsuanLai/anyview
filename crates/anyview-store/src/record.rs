//! Per-file view memory: the record stored for one file, the name it is stored under, whether it
//! still applies to the file on disk, and whether pruning keeps it.

use crate::error::StoreError;
use crate::history::History;
use anyview_core::{FilePath, FileStamp, Resume};

/// What is stored for one file: the memory, with the path and fingerprint it was taken at.
///
/// The fingerprint is the file's length and modification time, exactly equal. It is cheap (one
/// `stat`) and catches a different file at the same path and an edited one. It also discards the
/// memory of a file that was only touched or restored from a backup, which is the safe side:
/// opening at the start costs a scroll, restoring into the wrong document costs trust.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ResumeRecord {
    /// The file this was remembered for; checked on load because the file name is a hash.
    pub path: FilePath,
    /// The file as it was when remembered.
    pub stamp: FileStamp,
    /// Where it was left.
    pub resume: Resume,
}

/// The name a path's record is stored under: its FNV-1a hash in 16 hex digits. FNV is written out
/// here because the standard hasher's output is not promised across Rust versions, and a changed
/// name would orphan every record. A collision is harmless: the record names its path, and a load
/// for another path finds a mismatch and reports nothing remembered.
pub(crate) fn resume_file_name(path: &FilePath) -> Result<String, StoreError> {
    let text = path
        .as_path()
        .to_str()
        .ok_or_else(|| StoreError::PathNotUtf8 {
            path: path.as_path().to_path_buf(),
        })?;
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    Ok(format!("{hash:016x}.json"))
}

/// The memory in `record` if it was taken for `path` and the file still looks the same
/// (`current` is the stamp the file has now).
pub(crate) fn applicable(
    record: ResumeRecord,
    path: &FilePath,
    current: FileStamp,
) -> Option<Resume> {
    (record.path == *path && record.stamp == current).then_some(record.resume)
}

/// Why a record is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PruneReason {
    /// The file no longer exists.
    Vanished,
    /// A different file is at the path now.
    Replaced,
    /// The history no longer holds the file, so nothing will ask for its memory.
    Forgotten,
}

/// What pruning does with a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prune {
    /// Leave it.
    Keep,
    /// Delete it, for this reason.
    Remove(PruneReason),
}

/// Whether `record` stays: its file must still exist (`current` is its stamp now, `None` if
/// absent) with the stamp it was taken at, and be in `history`. The history bound keeps the
/// number of records at the history's cap.
pub(crate) fn prune_decision(
    record: &ResumeRecord,
    history: &History,
    current: Option<FileStamp>,
) -> Prune {
    match current {
        None => Prune::Remove(PruneReason::Vanished),
        Some(stamp) if stamp != record.stamp => Prune::Remove(PruneReason::Replaced),
        Some(_) if !history.entries.iter().any(|e| e.path == record.path) => {
            Prune::Remove(PruneReason::Forgotten)
        }
        Some(_) => Prune::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryEntry;
    use crate::label::ResumeLabel;
    use crate::viewed::Viewed;
    use anyview_core::{ByteLen, FormatKind, LineIndex, ModTime};

    fn stamp(len: u64, modified: i64) -> FileStamp {
        FileStamp {
            len: ByteLen(len),
            modified: ModTime(modified),
        }
    }

    fn path(text: &str) -> FilePath {
        FilePath::new(text).unwrap()
    }

    fn record(at: &str, stamp: FileStamp) -> ResumeRecord {
        ResumeRecord {
            path: path(at),
            stamp,
            resume: Resume::Text { line: LineIndex(4) },
        }
    }

    fn history_of(paths: &[&str]) -> History {
        History {
            entries: paths
                .iter()
                .map(|p| HistoryEntry {
                    path: path(p),
                    kind: FormatKind::PlainText,
                    viewed: Viewed(1),
                    label: ResumeLabel::Unlabelled,
                })
                .collect(),
        }
    }

    #[test]
    fn a_record_round_trips_in_its_stored_form() {
        let stored = record("/a.txt", stamp(10, -5));
        let json = r#"{"path":"/a.txt","stamp":{"len":10,"modified":-5},"resume":{"kind":"text","v":{"line":4}}}"#;
        assert_eq!(serde_json::to_string(&stored).unwrap(), json);
        assert_eq!(serde_json::from_str::<ResumeRecord>(json).unwrap(), stored);
    }

    #[test]
    fn file_names_are_stable_hex_and_differ_by_path() {
        // The hash of "/a" is pinned: a changed algorithm would orphan every stored record.
        assert_eq!(
            resume_file_name(&path("/a")).unwrap(),
            "07d66707b49cd92d.json"
        );
        assert_ne!(
            resume_file_name(&path("/a")).unwrap(),
            resume_file_name(&path("/b")).unwrap()
        );
        assert_eq!(
            resume_file_name(&path("/a/./b/../c")).unwrap(),
            resume_file_name(&path("/a/c")).unwrap()
        );
    }

    #[test]
    fn a_record_applies_only_to_its_path_and_fingerprint() {
        // (name, record path, record stamp, asked path, current stamp, applies)
        type Row = (
            &'static str,
            &'static str,
            (u64, i64),
            &'static str,
            (u64, i64),
            bool,
        );
        const CASES: &[Row] = &[
            ("same file", "/a", (10, 5), "/a", (10, 5), true),
            ("edited: length", "/a", (10, 5), "/a", (11, 5), false),
            ("edited: time", "/a", (10, 5), "/a", (10, 6), false),
            ("hash collision", "/a", (10, 5), "/b", (10, 5), false),
        ];
        for (name, at, was, asked, now, applies) in CASES {
            let got = applicable(
                record(at, stamp(was.0, was.1)),
                &path(asked),
                stamp(now.0, now.1),
            );
            assert_eq!(got.is_some(), *applies, "{name}");
            if *applies {
                assert_eq!(got, Some(Resume::Text { line: LineIndex(4) }), "{name}");
            }
        }
    }

    #[test]
    fn pruning_keeps_only_present_unchanged_remembered_files() {
        // (name, current stamp of /a, history paths, decision)
        type Row = (
            &'static str,
            Option<(u64, i64)>,
            &'static [&'static str],
            Prune,
        );
        const CASES: &[Row] = &[
            (
                "unchanged and in history",
                Some((10, 5)),
                &["/a"],
                Prune::Keep,
            ),
            (
                "file gone",
                None,
                &["/a"],
                Prune::Remove(PruneReason::Vanished),
            ),
            (
                "gone and not in history reports the file first",
                None,
                &[],
                Prune::Remove(PruneReason::Vanished),
            ),
            (
                "replaced",
                Some((99, 5)),
                &["/a"],
                Prune::Remove(PruneReason::Replaced),
            ),
            (
                "touched",
                Some((10, 6)),
                &["/a"],
                Prune::Remove(PruneReason::Replaced),
            ),
            (
                "fell out of history",
                Some((10, 5)),
                &["/b"],
                Prune::Remove(PruneReason::Forgotten),
            ),
        ];
        for (name, current, in_history, want) in CASES {
            let got = prune_decision(
                &record("/a", stamp(10, 5)),
                &history_of(in_history),
                current.map(|(len, time)| stamp(len, time)),
            );
            assert_eq!(got, *want, "{name}");
        }
    }
}
