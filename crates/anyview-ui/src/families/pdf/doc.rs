//! An opened PDF: the document every worker shares, its outline, the rows of the Info tab, and the
//! small pool of renderer scratch the workers borrow. Opening reads and parses the whole file, so
//! it runs on a worker (`StageView::open`); the UI thread only holds the result.

use crate::families::view::Area;
use crate::io::OpenError;
use crate::{EditCaution, EditOffer};
use anyview_core::{BookFormat, Facts, FormatDetail, PageCount, PageIndex, Sniffed, Source};
use anyview_machines::{OutlineFold, OutlineShape};
use anyview_pdf::{Disclosure, OutlineEntry, PageSize, PdfDocument, PdfError, PdfWorker, outline};
use std::io::ErrorKind;
use std::sync::{Mutex, PoisonError};

/// The most renderer scratch kept between jobs: one per worker that has drawn for this document.
const SCRATCH_KEPT: usize = 16;

/// Why a PDF did not open, in the terms the window acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfFailure {
    /// The file system refused.
    Unreadable(ErrorKind),
    /// The document is encrypted; the viewer has no way to ask for a password yet.
    Locked,
    /// The document has no pages to show.
    Empty,
    /// The document could not be parsed.
    Damaged,
}

impl std::fmt::Display for PdfFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfFailure::Unreadable(kind) => write!(f, "cannot read the PDF: {kind}"),
            PdfFailure::Locked => f.write_str("the PDF needs a password"),
            PdfFailure::Empty => f.write_str("the PDF has no pages"),
            PdfFailure::Damaged => f.write_str("the PDF is damaged"),
        }
    }
}

impl std::error::Error for PdfFailure {}

impl PdfFailure {
    /// What `error` means to the window.
    fn of(error: &PdfError) -> PdfFailure {
        match error {
            PdfError::Locked => PdfFailure::Locked,
            PdfError::NoPages => PdfFailure::Empty,
            PdfError::Io(error) => PdfFailure::Unreadable(error.kind()),
            PdfError::Pdf(_)
            | PdfError::Edit(_)
            | PdfError::Core(_)
            | PdfError::PageOutOfRange { .. }
            | PdfError::WouldDeleteAll
            | PdfError::PagesChanged { .. }
            | PdfError::EditUnsupported { .. }
            | PdfError::Stopped => PdfFailure::Damaged,
        }
    }
}

/// Where an opened PDF came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfOrigin {
    /// A PDF file.
    File,
    /// An EPUB bound as a PDF in the window, which opens at reading width and takes no page edit.
    Book,
    /// A comic bound as a PDF in the window: its pages are pictures, so it opens on the whole
    /// page, and it takes no page edit.
    Comic,
}

/// An opened PDF. Shared in an `Arc` by the window and every job; nothing in it changes.
#[derive(Debug)]
pub struct PdfDoc {
    /// The document, shared by every worker.
    pub document: PdfDocument,
    /// The bookmarks in reading order; empty when the file has none.
    pub outline: Vec<OutlineEntry>,
    /// How deep each bookmark sits: what the outline's fold machine reads of the document.
    shape: OutlineShape,
    /// The outline as the file asks it to first open, with the bookmarks it folds.
    opening: OutlineFold,
    /// The rows of the Info tab.
    pub facts: Facts,
    /// A signed document asks before a page edit rewrites it.
    pub offer: EditOffer,
    /// Whether this is a PDF file or a book bound as one.
    pub origin: PdfOrigin,
    scratch: Mutex<Vec<PdfWorker>>,
}

impl PdfDoc {
    /// The document `document` is: its outline read, its Document section made from what the file
    /// says of itself. The General section (kind, size, dates) is the window's.
    pub(super) fn of(document: PdfDocument) -> PdfDoc {
        PdfDoc::with_facts(document.info().facts(), document)
    }

    /// The document a book was bound as, with the rows the book's own Info tab has (not the bound
    /// PDF's Info dictionary, which is the binder's). Its pages take no edit.
    pub(super) fn of_book(document: PdfDocument, facts: Facts, origin: PdfOrigin) -> PdfDoc {
        // The book's pages are a PDF only in the window: no edit of them could be written back.
        PdfDoc {
            offer: EditOffer::Withheld,
            origin,
            ..PdfDoc::with_facts(facts, document)
        }
    }

    fn with_facts(facts: Facts, document: PdfDocument) -> PdfDoc {
        let offer = if document.is_signed() {
            EditOffer::Asks(EditCaution::Signed)
        } else {
            EditOffer::Plain
        };
        let outline = outline(&document);
        PdfDoc {
            offer,
            origin: PdfOrigin::File,
            shape: OutlineShape::of(outline.iter().map(|entry| entry.depth)),
            opening: OutlineFold::folded(
                (0_u32..)
                    .zip(&outline)
                    .filter(|(_, entry)| entry.disclosure == Disclosure::Closed)
                    .map(|(at, _)| at),
            ),
            outline,
            document,
            facts,
            scratch: Mutex::default(),
        }
    }

    /// How deep each bookmark sits.
    pub(super) fn shape(&self) -> &OutlineShape {
        &self.shape
    }

    /// The outline's folds before the person touches one.
    pub(super) fn opening(&self) -> &OutlineFold {
        &self.opening
    }

    /// The page count.
    #[must_use]
    pub fn pages(&self) -> PageCount {
        self.document.page_count()
    }

    /// Every page's displayed size.
    #[must_use]
    pub fn sizes(&self) -> &[PageSize] {
        self.document.page_sizes()
    }

    /// Runs `work` with a renderer scratch no other job holds. The pool of scratch is the doc's
    /// own because the job seam gives a worker no state of its own: the first job of a worker
    /// makes one, a finished job returns it, so a worker's next batch of the same page and zoom
    /// finds the page it already read.
    pub(super) fn with_scratch<T>(&self, work: impl FnOnce(&mut PdfWorker) -> T) -> T {
        let taken = self
            .scratch
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let mut worker = taken.unwrap_or_default();
        let done = work(&mut worker);
        let mut kept = self.scratch.lock().unwrap_or_else(PoisonError::into_inner);
        if kept.len() < SCRATCH_KEPT {
            kept.push(worker);
        }
        done
    }

    /// The size of `page`, the first page's when there is no such page.
    #[must_use]
    pub fn size_of(&self, page: PageIndex) -> PageSize {
        let sizes = self.sizes();
        usize::try_from(page.0)
            .ok()
            .and_then(|at| sizes.get(at))
            .or_else(|| sizes.first())
            .copied()
            .unwrap_or(PageSize::LETTER)
    }
}

/// Open the PDF `src`. Blocking.
pub(super) fn open(src: &Source) -> Result<PdfDoc, OpenError> {
    let document = PdfDocument::open(src.path().as_path())
        .map_err(|error| OpenError::Pdf(PdfFailure::of(&error)))?;
    Ok(PdfDoc::of(document))
}

/// Open the book `src` as the PDF it is bound as. Blocking: reads it and lays it out, unless this
/// session has bound this version of it already.
pub(super) fn open_book(src: &Source, sniffed: &Sniffed) -> Result<PdfDoc, OpenError> {
    let bound = super::bound::bind(src, sniffed)?;
    let origin = match sniffed.detail() {
        FormatDetail::Book(BookFormat::Cbz) => PdfOrigin::Comic,
        FormatDetail::None
        | FormatDetail::Raster(_)
        | FormatDetail::Code(_)
        | FormatDetail::Table(_)
        | FormatDetail::Tree(_)
        | FormatDetail::Text(_)
        | FormatDetail::Media(_)
        | FormatDetail::Font(_)
        | FormatDetail::Archive(_)
        | FormatDetail::Book(BookFormat::Epub)
        | FormatDetail::Office(_) => PdfOrigin::Book,
    };
    Ok(PdfDoc::of_book(bound.document, bound.facts, origin))
}

/// The room a window gives the pages, as the PDF stage reads it.
pub(super) fn room_of(area: Option<Area>) -> Option<Area> {
    area.filter(|area| area.size.width.0 >= 1.0 && area.size.height.0 >= 1.0)
}
