//! The books bound this session, by file and stamp. Binding costs a layout of every chapter;
//! keeping the bytes makes the next open of the same version of the file instant. A few books are
//! kept, the oldest let go first, and never more than `BYTES` in all. Nothing is written to disk:
//! the host offers the views no folder of its own to keep a cache in, and a book that changed on
//! disk has a new stamp, so it never finds a stale layout.

use anyview_core::{Facts, Source};
use std::sync::{Arc, Mutex, PoisonError};

/// The most books kept.
const BOOKS: usize = 4;

/// The most bytes kept in all, the book that was just bound included.
const BYTES: usize = 256 * 1024 * 1024;

struct Entry {
    source: Source,
    bytes: Arc<[u8]>,
    facts: Facts,
}

/// A set of bound books, newest use last. The session has one, [`KEPT`]; a test makes its own,
/// so tests running side by side do not let each other's books go.
struct Kept(Mutex<Vec<Entry>>);

static KEPT: Kept = Kept::new();

impl Kept {
    const fn new() -> Kept {
        Kept(Mutex::new(Vec::new()))
    }

    fn get(&self, src: &Source) -> Option<(Arc<[u8]>, Facts)> {
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let at = kept.iter().position(|entry| &entry.source == src)?;
        // The newest use goes last, so the oldest is the first to go.
        let entry = kept.remove(at);
        let found = (Arc::clone(&entry.bytes), entry.facts.clone());
        kept.push(entry);
        Some(found)
    }

    fn put(&self, src: &Source, bytes: Vec<u8>, facts: &Facts) -> Arc<[u8]> {
        let bytes: Arc<[u8]> = bytes.into();
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        kept.retain(|entry| entry.source.path() != src.path());
        kept.push(Entry {
            source: src.clone(),
            bytes: Arc::clone(&bytes),
            facts: facts.clone(),
        });
        while kept.len() > BOOKS
            || (kept.len() > 1 && kept.iter().map(|entry| entry.bytes.len()).sum::<usize>() > BYTES)
        {
            kept.remove(0);
        }
        bytes
    }
}

/// The bound book kept for this version of `src`.
pub(super) fn get(src: &Source) -> Option<(Arc<[u8]>, Facts)> {
    KEPT.get(src)
}

/// Keep `bytes` as the bound book of `src`, and hand back the shared copy.
pub(super) fn put(src: &Source, bytes: Vec<u8>, facts: &Facts) -> Arc<[u8]> {
    KEPT.put(src, bytes, facts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ByteLen, FilePath, FileStamp, ModTime};

    fn source(name: &str, len: u64) -> Source {
        let path = FilePath::new(format!("/kept-test/{name}")).expect("an absolute path");
        let stamp = FileStamp {
            len: ByteLen(len),
            modified: ModTime(1),
        };
        Source::new(path, stamp)
    }

    #[test]
    fn a_book_is_found_again_until_the_file_changes() {
        let (a, changed) = (source("a.epub", 10), source("a.epub", 11));
        let (kept, facts) = (Kept::new(), Facts::empty());
        kept.put(&a, vec![1, 2, 3], &facts);
        assert_eq!(
            kept.get(&a).map(|(bytes, _)| bytes.to_vec()),
            Some(vec![1, 2, 3])
        );
        assert!(kept.get(&changed).is_none(), "a new stamp is a new book");
    }

    #[test]
    fn the_oldest_book_goes_first() {
        let (kept, facts) = (Kept::new(), Facts::empty());
        let names: Vec<Source> = (0..BOOKS + 1)
            .map(|at| source(&format!("lru-{at}.epub"), 1))
            .collect();
        for src in &names {
            kept.put(src, vec![0], &facts);
        }
        assert!(kept.get(&names[0]).is_none(), "the first was let go");
        assert!(kept.get(&names[BOOKS]).is_some(), "the last is kept");
    }
}
