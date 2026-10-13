//! Finding text across the document, a page at a time: the page is the unit of work, so a stop
//! takes effect between pages and the hits found so far are kept. A phrase that begins at the end
//! of one page and ends on the next is a hit of the page it begins on. The text of the pages
//! searched is kept in the worker for the next search.

mod snippet;
mod texts;

pub use snippet::Snippet;
pub(crate) use texts::PageTexts;

use crate::document::PdfDocument;
use crate::error::PdfError;
use crate::geometry::{PageRect, displayed};
use crate::halt::Halt;
use crate::render::{End, PdfWorker};
use anyview_core::PageIndex;
use anyview_core::work::Stop;
use anyview_core::{CaseMatch, FindOptions, WordMatch};
use pdfrum::{CharIndex, FindOptions as PageFind, Rect, Rotation, TextIndex, TextPage};
use std::ops::Range;
use std::sync::Arc;

/// What to look for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    /// The text. It is split at spaces: the pieces must appear in order, so a phrase is found
    /// across a line break, and across a page break.
    pub text: String,
    /// Whether case and whole words matter.
    pub options: FindOptions,
}

impl SearchQuery {
    /// A query for `text`, ignoring case, finding parts of words.
    pub fn new(text: impl Into<String>) -> SearchQuery {
        SearchQuery {
            text: text.into(),
            options: FindOptions::default(),
        }
    }

    /// The same query matched as `options` say.
    #[must_use]
    pub fn with_options(self, options: FindOptions) -> SearchQuery {
        SearchQuery { options, ..self }
    }

    fn page_find(&self) -> PageFind {
        PageFind {
            match_case: matches!(self.options.case, CaseMatch::Exact),
            match_whole_word: matches!(self.options.words, WordMatch::Whole),
            consecutive: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// One place the text was found: a page, the rectangles that mark it (more than one when it
/// wraps onto a second line) and the words around it. A phrase over a page break is on the page it
/// begins on, and its rectangles mark only the part of it there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The page.
    pub page: PageIndex,
    /// Where on it, as fractions of the page as displayed.
    pub rects: Vec<PageRect>,
    /// The words around it.
    pub snippet: Snippet,
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

/// A page's text as pdfrum extracted it, with what turns a stretch of the text into rectangles.
struct Extracted {
    text: TextPage,
    crop: Rect,
    rotation: Rotation,
}

impl Extracted {
    fn read(doc: &PdfDocument, worker: &mut PdfWorker, page: PageIndex) -> Result<Self, PdfError> {
        let sheet = doc.page(page)?;
        let text = sheet.text_on(worker.session(doc));
        Ok(Extracted {
            text,
            crop: sheet.crop_box(),
            rotation: sheet.rotation(),
        })
    }

    /// The rectangles of the stretch `span` of the page's search text, on the page as displayed.
    fn rects(&self, span: Range<usize>) -> Vec<PageRect> {
        let (Some(first), Some(last)) = (
            self.text.runs.char_index(TextIndex::new(span.start)),
            span.end
                .checked_sub(1)
                .and_then(|end| self.text.runs.char_index(TextIndex::new(end))),
        ) else {
            return Vec::new();
        };
        let range = first..CharIndex::new(last.get() + 1);
        self.text
            .rects(range)
            .into_iter()
            .map(|rect| displayed(rect, self.crop, self.rotation))
            .collect()
    }
}

/// A page as a search reads it: its search text, from the worker's keep or read now, and the page
/// as extracted when this search had to read it (a hit needs it for its rectangles).
struct Seen {
    chars: Arc<[char]>,
    read: Option<Extracted>,
}

impl Seen {
    fn of(doc: &PdfDocument, worker: &mut PdfWorker, page: PageIndex) -> Result<Seen, PdfError> {
        if let Some(chars) = worker.texts.get(doc.id(), page) {
            return Ok(Seen { chars, read: None });
        }
        let read = Extracted::read(doc, worker, page)?;
        let chars: Arc<[char]> = Arc::from(read.text.search_text.as_slice());
        worker.texts.keep(doc.id(), page, Arc::clone(&chars));
        Ok(Seen {
            chars,
            read: Some(read),
        })
    }

    /// The page as extracted, read now if the search text came from the keep.
    fn extracted(
        &mut self,
        doc: &PdfDocument,
        worker: &mut PdfWorker,
        page: PageIndex,
    ) -> Result<&Extracted, PdfError> {
        let read = match self.read.take() {
            Some(read) => read,
            None => Extracted::read(doc, worker, page)?,
        };
        let read: &Extracted = self.read.insert(read);
        Ok(read)
    }
}

/// The stretches of `chars` that `query` matches.
fn matches(chars: &[char], query: &SearchQuery) -> Vec<Range<usize>> {
    let page = TextPage {
        search_text: chars.to_vec(),
        ..TextPage::default()
    };
    page.find_with(&query.text, query.page_find())
        .map(|found| found.start.get()..found.end.get())
        .collect()
}

/// A hit, and where in its page's text it starts.
struct Placed {
    at: usize,
    hit: Hit,
}

/// The hits of `query` that lie within one page.
fn page_hits(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    page: PageIndex,
    seen: &mut Seen,
    query: &SearchQuery,
) -> Result<Vec<Placed>, PdfError> {
    let found = matches(&seen.chars, query);
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let chars = Arc::clone(&seen.chars);
    let read = seen.extracted(doc, worker, page)?;
    let mut hits = Vec::new();
    for span in found {
        let rects = read.rects(span.clone());
        if !rects.is_empty() {
            hits.push(Placed {
                at: span.start,
                hit: Hit {
                    page,
                    rects,
                    snippet: Snippet::around(&chars, span.start, span.end),
                },
            });
        }
    }
    Ok(hits)
}

/// The hits of `query` on one page.
pub fn search_page(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    page: PageIndex,
    query: &SearchQuery,
) -> Result<Vec<Hit>, PdfError> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = Seen::of(doc, worker, page)?;
    let placed = page_hits(doc, worker, page, &mut seen, query)?;
    Ok(placed.into_iter().map(|placed| placed.hit).collect())
}

/// A page whose hits are not yet in the answer: the page after it may add the hits that begin on
/// it and end on that one.
struct Behind {
    page: PageIndex,
    seen: Seen,
    hits: Vec<Placed>,
}

/// A match that begins at the end of one page and ends at the start of the next: where it begins
/// in the first page's text, and the words around it.
struct Across {
    at: usize,
    snippet: Snippet,
}

/// The matches of `query` over the break between a page whose text is `before` and the one whose
/// text is `after`. Only the ends of the two are read: a match is no longer than the query and the
/// spaces between its words.
fn across(before: &[char], after: &[char], query: &SearchQuery) -> Vec<Across> {
    let reach = query
        .text
        .chars()
        .count()
        .saturating_mul(2)
        .saturating_add(32);
    let from = before.len().saturating_sub(reach);
    let tail = before.get(from..).unwrap_or_default();
    let head = after.get(..reach.min(after.len())).unwrap_or_default();
    if tail.is_empty() || head.is_empty() {
        return Vec::new();
    }
    let mut joined: Vec<char> = Vec::with_capacity(tail.len() + head.len() + 1);
    joined.extend_from_slice(tail);
    joined.push('\n');
    joined.extend_from_slice(head);
    matches(&joined, query)
        .into_iter()
        // Whatever lies within one of the two pages is that page's own hit.
        .filter(|span| span.start < tail.len() && span.end > tail.len() + 1)
        .map(|span| Across {
            at: from + span.start,
            snippet: Snippet::around(&joined, span.start, span.end),
        })
        .collect()
}

impl Behind {
    /// Add the hits that begin on this page and end on the page whose text is `after`, in their
    /// place among the page's own, and give the hits.
    fn closed(
        mut self,
        doc: &PdfDocument,
        worker: &mut PdfWorker,
        after: &[char],
        query: &SearchQuery,
    ) -> Vec<Hit> {
        let spanning = across(&self.seen.chars, after, query);
        let (page, end) = (self.page, self.seen.chars.len());
        if !spanning.is_empty()
            && let Ok(read) = self.seen.extracted(doc, worker, page)
        {
            for over in spanning {
                let rects = read.rects(over.at..end);
                if !rects.is_empty() {
                    self.hits.push(Placed {
                        at: over.at,
                        hit: Hit {
                            page,
                            rects,
                            snippet: over.snippet,
                        },
                    });
                }
            }
            self.hits.sort_by_key(|placed| placed.at);
        }
        self.into_hits()
    }

    fn into_hits(self) -> Vec<Hit> {
        self.hits.into_iter().map(|placed| placed.hit).collect()
    }
}

/// The hits of `query` in the whole document, page by page. A raised stop ends the search between
/// pages (and inside a page's interpretation, never mid-way through extraction), keeping the hits
/// of the pages searched. A page that will not load has no hits, and no phrase runs over it.
pub fn search_document(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    query: &SearchQuery,
    stop: &Stop,
) -> (Hits, End) {
    let halt = Halt::new(stop);
    let mut found: Vec<Hit> = Vec::new();
    if query.is_empty() {
        let end = if halt.is_up() {
            End::Stopped
        } else {
            End::Complete
        };
        return (Hits(found), end);
    }
    let mut behind: Option<Behind> = None;
    for at in 0..doc.page_count().get() {
        if halt.is_up() {
            found.extend(behind.take().map(Behind::into_hits).unwrap_or_default());
            return (Hits(found), End::Stopped);
        }
        let page = PageIndex(at);
        let Ok(mut seen) = Seen::of(doc, worker, page) else {
            found.extend(behind.take().map(Behind::into_hits).unwrap_or_default());
            continue;
        };
        let hits = page_hits(doc, worker, page, &mut seen, query).unwrap_or_default();
        if let Some(before) = behind.take() {
            found.extend(before.closed(doc, worker, &seen.chars, query));
        }
        behind = Some(Behind { page, seen, hits });
    }
    found.extend(behind.take().map(Behind::into_hits).unwrap_or_default());
    (Hits(found), End::Complete)
}
