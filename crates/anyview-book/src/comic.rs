//! A comic: a zip of images read one page at a time, in natural order.

use crate::error::BookError;
use crate::natural::natural_order;
use crate::zip_read::{file_names, read};
use anyview_core::{ByteLen, FilePath, SectionCount, SectionIndex};

/// The largest page that is read.
const PAGE_LIMIT: ByteLen = ByteLen(64 * 1024 * 1024);

/// One page of a comic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComicPage {
    /// The entry that holds it.
    pub entry: String,
    /// The media type its extension names.
    pub mime: &'static str,
}

/// An opened comic: where it is and its pages in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comic {
    path: FilePath,
    pages: Vec<ComicPage>,
    count: SectionCount,
}

fn mime_of(entry: &str) -> Option<&'static str> {
    let name = entry.rsplit('/').next().unwrap_or(entry);
    if name.starts_with('.') || entry.starts_with("__MACOSX/") {
        return None;
    }
    match name.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

impl Comic {
    /// The comic in the zip at `path`: its images, sorted. A zip with none is [`BookError::Empty`].
    pub fn open(path: &FilePath) -> Result<Comic, BookError> {
        let mut pages: Vec<ComicPage> = file_names(path)?
            .into_iter()
            .filter_map(|entry| mime_of(&entry).map(|mime| ComicPage { entry, mime }))
            .collect();
        let count = u32::try_from(pages.len())
            .ok()
            .and_then(SectionCount::new)
            .ok_or(BookError::Empty)?;
        pages.sort_by(|a, b| natural_order(&a.entry, &b.entry));
        Ok(Comic {
            path: path.clone(),
            pages,
            count,
        })
    }

    /// How many pages there are.
    pub fn sections(&self) -> SectionCount {
        self.count
    }

    /// The pages in reading order.
    pub fn pages(&self) -> &[ComicPage] {
        &self.pages
    }

    /// The page `at` and its bytes. Blocking.
    pub fn page(&self, at: SectionIndex) -> Result<(&ComicPage, Vec<u8>), BookError> {
        let page = self
            .pages
            .get(at.0 as usize)
            .ok_or(BookError::NoSuchSection)?;
        Ok((page, read(&self.path, &page.entry, PAGE_LIMIT)?))
    }
}
