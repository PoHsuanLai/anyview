//! An opened PDF: the document every worker shares, its outline, the rows of the Info tab, and the
//! small pool of renderer scratch the workers borrow. Opening reads and parses the whole file, so
//! it runs on a worker (`StageView::open`); the UI thread only holds the result.

use crate::families::view::Area;
use crate::io::OpenError;
use anyview_core::{ByteLen, FactLabel, FactValue, Facts, PageCount, PageIndex, Source};
use anyview_pdf::{OutlineEntry, PageSize, PdfDocument, PdfError, PdfWorker, outline};
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
            | PdfError::EditUnsupported { .. }
            | PdfError::Stopped => PdfFailure::Damaged,
        }
    }
}

/// An opened PDF. Shared in an `Arc` by the window and every job; nothing in it changes.
#[derive(Debug)]
pub struct PdfDoc {
    /// The document, shared by every worker.
    pub document: PdfDocument,
    /// The bookmarks in reading order; empty when the file has none.
    pub outline: Vec<OutlineEntry>,
    /// The rows of the Info tab.
    pub facts: Facts,
    scratch: Mutex<Vec<PdfWorker>>,
}

impl PdfDoc {
    /// The document `document` is, `size` bytes on disk: its outline read, its Info rows made.
    pub(super) fn of(document: PdfDocument, size: ByteLen) -> PdfDoc {
        let facts = Facts::empty()
            .with(FactLabel::Kind, FactValue::text("application/pdf"))
            .with(FactLabel::Pages, FactValue::pages(document.page_count()))
            .with(FactLabel::Size, FactValue::size(size));
        PdfDoc {
            outline: outline(&document),
            document,
            facts,
            scratch: Mutex::default(),
        }
    }

    /// The page count.
    pub fn pages(&self) -> PageCount {
        self.document.page_count()
    }

    /// Every page's displayed size.
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
    Ok(PdfDoc::of(document, src.stamp().len))
}

/// The room a window gives the pages, as the PDF stage reads it.
pub(super) fn room_of(area: Option<Area>) -> Option<Area> {
    area.filter(|area| area.size.width.0 >= 1.0 && area.size.height.0 >= 1.0)
}
