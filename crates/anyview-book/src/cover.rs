//! The cover of a book: one image read out of the zip, never decoded here.

use crate::comic::Comic;
use crate::epub::Epub;
use anyview_core::{ByteLen, FilePath, SectionIndex};

/// The largest cover that is read.
const COVER_LIMIT: ByteLen = ByteLen(16 * 1024 * 1024);

/// A cover image as the zip holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    /// The entry's file name, which says what format the bytes are.
    pub name: String,
    /// The image file's bytes.
    pub bytes: Vec<u8>,
}

/// The cover the package of the EPUB `epub` names. Blocking.
pub fn epub_cover(epub: &Epub) -> Option<Cover> {
    let entry = epub.cover_entry()?;
    let bytes = epub.entry(entry, COVER_LIMIT).ok()?;
    Some(Cover {
        name: entry.rsplit('/').next().unwrap_or(entry).to_owned(),
        bytes,
    })
}

/// The first page of the comic at `path`. Blocking.
pub fn comic_cover(path: &FilePath) -> Option<Cover> {
    let comic = Comic::open(path).ok()?;
    let (page, bytes) = comic.page(SectionIndex(0)).ok()?;
    Some(Cover {
        name: page
            .entry
            .rsplit('/')
            .next()
            .unwrap_or(&page.entry)
            .to_owned(),
        bytes,
    })
}
