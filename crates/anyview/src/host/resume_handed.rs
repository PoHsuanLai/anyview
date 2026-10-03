//! A place the launcher handed over, read once for the file it names and then left to the store.

use anyview_core::{FilePath, FileStamp, Resume};
use anyview_ui::ResumeSource;
use std::sync::{Arc, Mutex, PoisonError};

/// A [`ResumeSource`] that answers the first question about `file` with the place the launcher's
/// pane held, and every other question (and every later one about `file`) from `inner`.
pub struct HandedResume {
    file: FilePath,
    place: Mutex<Option<Resume>>,
    inner: Arc<dyn ResumeSource>,
}

impl HandedResume {
    /// `place` for `file` the first time it is asked, `inner` otherwise.
    pub fn new(file: FilePath, place: Resume, inner: Arc<dyn ResumeSource>) -> Self {
        HandedResume {
            file,
            place: Mutex::new(Some(place)),
            inner,
        }
    }
}

impl std::fmt::Debug for HandedResume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandedResume")
            .field("file", &self.file)
            .finish_non_exhaustive()
    }
}

impl ResumeSource for HandedResume {
    fn recall(&self, path: &FilePath, stamp: FileStamp) -> Resume {
        if *path == self.file {
            // A poisoned lock still holds the place: only `take` writes, and it cannot leave it
            // half written.
            let handed = self
                .place
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(place) = handed {
                return place;
            }
        }
        self.inner.recall(path, stamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ByteLen, LineIndex, ModTime};

    #[derive(Debug)]
    struct Stored(Resume);

    impl ResumeSource for Stored {
        fn recall(&self, _: &FilePath, _: FileStamp) -> Resume {
            self.0.clone()
        }
    }

    fn stamp() -> FileStamp {
        FileStamp {
            len: ByteLen(1),
            modified: ModTime(0),
        }
    }

    #[test]
    fn the_handed_place_is_read_once_for_its_file_and_the_store_answers_the_rest() {
        let file = FilePath::new("/a.txt").unwrap();
        let other = FilePath::new("/b.txt").unwrap();
        let handed = Resume::Text {
            line: LineIndex(40),
        };
        let stored = Resume::Text { line: LineIndex(7) };
        let source = HandedResume::new(
            file.clone(),
            handed.clone(),
            Arc::new(Stored(stored.clone())),
        );
        // (who is asked, what comes back)
        let asked = [
            ("another file", &other, &stored),
            ("the file, first", &file, &handed),
            ("the file again", &file, &stored),
        ];
        for (name, path, want) in asked {
            assert_eq!(source.recall(path, stamp()), *want, "{name}");
        }
    }
}
