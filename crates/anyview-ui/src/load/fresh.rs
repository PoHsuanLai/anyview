//! Whether a file on disk is still the one that was opened: the decision behind a reload.

use anyview_core::FileStamp;

/// What a look at the file on disk says about the one on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// The file is as it was opened: leave it be.
    Current,
    /// The file was edited or replaced: open it again.
    Changed,
}

/// What `seen`, the stamp the file has now, says about the file opened with stamp `open`. A file
/// that cannot be read at the moment (`None`: an editor saving through a rename leaves none for
/// an instant) is no change: what shows is still the last good copy, and the next event of the
/// watcher looks again.
pub fn freshness(open: FileStamp, seen: Option<FileStamp>) -> Freshness {
    match seen {
        Some(seen) if seen != open => Freshness::Changed,
        Some(_) | None => Freshness::Current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ByteLen, ModTime};

    const fn stamp(len: u64, modified: i64) -> FileStamp {
        FileStamp {
            len: ByteLen(len),
            modified: ModTime(modified),
        }
    }

    #[test]
    fn a_file_is_changed_when_its_stamp_is_not_the_one_it_was_opened_with() {
        // name, stamp when opened, stamp seen now, the verdict
        const CASES: &[(&str, FileStamp, Option<FileStamp>, Freshness)] = &[
            (
                "untouched",
                stamp(10, 5),
                Some(stamp(10, 5)),
                Freshness::Current,
            ),
            (
                "written later",
                stamp(10, 5),
                Some(stamp(10, 9)),
                Freshness::Changed,
            ),
            (
                "grown",
                stamp(10, 5),
                Some(stamp(12, 5)),
                Freshness::Changed,
            ),
            (
                "shrunk in the same second",
                stamp(10, 5),
                Some(stamp(3, 5)),
                Freshness::Changed,
            ),
            (
                "not readable for the moment",
                stamp(10, 5),
                None,
                Freshness::Current,
            ),
        ];
        for (name, open, seen, want) in CASES {
            assert_eq!(freshness(*open, *seen), *want, "{name}");
        }
    }
}
