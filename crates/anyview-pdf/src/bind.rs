//! Several PDFs bound into one, with a bookmark for each chapter: how a book laid out a chapter at
//! a time becomes the single document the PDF stage shows.

use crate::document::PdfDocument;
use crate::edit::save;
use crate::error::PdfError;
use crate::render::PdfWorker;
use crate::search::{SearchQuery, search_page};
use anyview_core::PageIndex;
use pdfrum_edit::{
    AnnotGoToView, BookmarkSpec, EditDoc, ImportOptions, PageRange, blank_document, import_pages,
    set_outline,
};

/// One bookmark of a bound document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    /// The text shown.
    pub title: String,
    /// How many levels down it sits; 0 is the top.
    pub depth: usize,
    /// The part it leads into.
    pub part: usize,
    /// When the part holds more than this one bookmark, the text that starts it: the page of the
    /// part that shows this text, no earlier than the page of the bookmark before it, is where the
    /// bookmark goes. A text that is not found leaves it on the page of the bookmark before it
    /// (or the part's first page).
    pub find: Option<String>,
}

/// `parts`, each a PDF, as the pages of one PDF in order, with `bookmarks` as its outline. A part
/// that is not a PDF is [`PdfError`]; a bookmark of a part that does not exist is left out.
pub fn bind(parts: &[Vec<u8>], bookmarks: &[Bookmark]) -> Result<Vec<u8>, PdfError> {
    let documents = parts
        .iter()
        .map(|bytes| PdfDocument::from_bytes(bytes.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let starts = starts_of(&documents);
    let joined = {
        let blank = blank_document(&[])?;
        let mut edit = EditDoc::new(&blank);
        for (document, start) in documents.iter().zip(&starts) {
            let pages = PageRange::all(document.page_count().get());
            let options = ImportOptions::builder().at(*start).build();
            import_pages(&mut edit, document.inner().parser(), &pages, &options)?;
        }
        save(&edit)?
    };
    if bookmarks.is_empty() {
        return Ok(joined);
    }
    let bound = PdfDocument::from_bytes(joined)?;
    let pages = place(&documents, &starts, bookmarks);
    let mut specs = Vec::with_capacity(pages.len());
    for (bookmark, page) in bookmarks.iter().zip(&pages) {
        let Some(page) = page else { continue };
        let reference = bound
            .inner()
            .parser()
            .page(page.0)
            .ok()
            .and_then(|dict| dict.reference);
        let spec = BookmarkSpec::new(bookmark.title.clone()).depth(bookmark.depth);
        specs.push(match reference {
            Some(reference) => spec.page(reference, AnnotGoToView::Fit),
            None => spec,
        });
    }
    let mut edit = EditDoc::new(bound.inner().parser());
    set_outline(&mut edit, &specs)?;
    save(&edit)
}

/// The page each part starts at.
fn starts_of(documents: &[PdfDocument]) -> Vec<u32> {
    let mut next = 0u32;
    documents
        .iter()
        .map(|document| {
            let start = next;
            next = next.saturating_add(document.page_count().get());
            start
        })
        .collect()
}

/// The page of the bound document each bookmark leads to; `None` for one whose part is not there.
fn place(
    documents: &[PdfDocument],
    starts: &[u32],
    bookmarks: &[Bookmark],
) -> Vec<Option<PageIndex>> {
    let mut worker = PdfWorker::default();
    let mut last: Option<(usize, u32)> = None;
    bookmarks
        .iter()
        .map(|bookmark| {
            let document = documents.get(bookmark.part)?;
            let start = *starts.get(bookmark.part)?;
            let from = match last {
                Some((part, page)) if part == bookmark.part => page,
                _ => 0,
            };
            let within = bookmark
                .find
                .as_deref()
                .and_then(|text| first_showing(document, &mut worker, text, from))
                .unwrap_or(from);
            last = Some((bookmark.part, within));
            Some(PageIndex(start.saturating_add(within)))
        })
        .collect()
}

/// The first page of `document`, from `from` on, that shows `text`.
fn first_showing(
    document: &PdfDocument,
    worker: &mut PdfWorker,
    text: &str,
    from: u32,
) -> Option<u32> {
    let query = SearchQuery::new(text);
    (from..document.page_count().get()).find(|page| {
        search_page(document, worker, PageIndex(*page), &query).is_ok_and(|hits| !hits.is_empty())
    })
}
