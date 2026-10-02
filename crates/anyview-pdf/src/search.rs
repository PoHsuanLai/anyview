//! Finding text across the document, a page at a time: the page is the unit of work, so a stop
//! takes effect between pages and the hits found so far are kept.

use crate::document::PdfDocument;
use crate::error::PdfError;
use crate::geometry::{PageRect, displayed};
use crate::halt::Halt;
use crate::render::{End, PdfWorker};
use crate::work_shim::Stop;
use anyview_core::PageIndex;
use pdfrum::{CharIndex, FindOptions};

/// Whether letter case matters to a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CaseMatch {
    /// `Fox` finds `fox` (the default).
    #[default]
    Ignore,
    /// `Fox` finds only `Fox`.
    Exact,
}

/// Whether a hit may be part of a longer word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WordMatch {
    /// `fox` finds the start of `foxes` (the default).
    #[default]
    Part,
    /// `fox` finds only the whole word.
    Whole,
}

/// What to look for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    /// The text. It is split at spaces: the pieces must appear in order, so a phrase is found
    /// across a line break.
    pub text: String,
    /// Case rule.
    pub case: CaseMatch,
    /// Word rule.
    pub words: WordMatch,
}

impl SearchQuery {
    /// A query for `text`, ignoring case, finding parts of words.
    pub fn new(text: impl Into<String>) -> SearchQuery {
        SearchQuery {
            text: text.into(),
            case: CaseMatch::Ignore,
            words: WordMatch::Part,
        }
    }

    fn options(&self) -> FindOptions {
        FindOptions {
            match_case: matches!(self.case, CaseMatch::Exact),
            match_whole_word: matches!(self.words, WordMatch::Whole),
            consecutive: false,
        }
    }
}

/// One place the text was found: a page and the rectangles that mark it (more than one when it
/// wraps onto a second line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The page.
    pub page: PageIndex,
    /// Where on it, as fractions of the page as displayed.
    pub rects: Vec<PageRect>,
}

/// The hits of one search, in document order: by page, then reading order. The viewer's find
/// machine holds only a count and a cursor and asks for a hit by its index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hits(Vec<Hit>);

impl Hits {
    /// How many hits there are.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the search found nothing.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The hit with this index.
    pub fn get(&self, index: u32) -> Option<&Hit> {
        usize::try_from(index).ok().and_then(|at| self.0.get(at))
    }

    /// Every hit, in order.
    pub fn iter(&self) -> impl Iterator<Item = &Hit> {
        self.0.iter()
    }

    /// The index of the first hit on `page` or after it, or of the last hit when none follows:
    /// the one nearest where the reader is. Zero when there are no hits.
    pub fn nearest(&self, page: PageIndex) -> u32 {
        let at = self.0.partition_point(|hit| hit.page < page);
        u32::try_from(at.min(self.0.len().saturating_sub(1))).unwrap_or(0)
    }
}

/// The hits of `query` on one page.
pub fn search_page(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    page: PageIndex,
    query: &SearchQuery,
) -> Result<Vec<Hit>, PdfError> {
    if query.text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let sheet = doc.page(page)?;
    let text = sheet.text_on(worker.session(doc));
    let (crop, rotation) = (sheet.crop_box(), sheet.rotation());
    let mut hits = Vec::new();
    for found in text.find_with(&query.text, query.options()) {
        let (Some(first), Some(last)) = (
            text.runs.char_index(found.start),
            found
                .end
                .get()
                .checked_sub(1)
                .and_then(|end| text.runs.char_index(end.into())),
        ) else {
            continue;
        };
        let range = first..CharIndex::new(last.get() + 1);
        let rects: Vec<PageRect> = text
            .rects(range)
            .into_iter()
            .map(|rect| displayed(rect, crop, rotation))
            .collect();
        if !rects.is_empty() {
            hits.push(Hit { page, rects });
        }
    }
    Ok(hits)
}

/// The hits of `query` in the whole document, page by page. A raised stop ends the search between
/// pages (and inside a page's interpretation, never mid-way through extraction), keeping the hits
/// of the pages searched. A page that will not load has no hits.
pub fn search_document(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    query: &SearchQuery,
    stop: &Stop,
) -> (Hits, End) {
    let halt = Halt::new(stop);
    let mut found = Vec::new();
    for at in 0..doc.page_count().get() {
        if halt.is_up() {
            return (Hits(found), End::Stopped);
        }
        if let Ok(hits) = search_page(doc, worker, PageIndex(at), query) {
            found.extend(hits);
        }
    }
    (Hits(found), End::Complete)
}
