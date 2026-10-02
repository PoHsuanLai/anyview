//! The job runner: the shape every back end shares (`run(doc, worker, job, stop) -> done`), for
//! the binary's pool to call. The crate spawns nothing and reads no clock of its own.

use crate::document::PdfDocument;
use crate::edit::{PageOp, apply};
use crate::error::PdfError;
use crate::export::{write_pages, write_text};
use crate::halt::Halt;
use crate::render::{End, PdfWorker, Raster, Tile, render_page, render_tiles};
use crate::search::{Hits, SearchQuery, search_document};
use crate::tile::TileBatch;
use anyview_core::work::{Backend, Stop, Ticket};
use anyview_core::{Dpi, PageIndex, PageSelection, TextFlavour};

/// One piece of blocking work. Every job carries the ticket its result comes back under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfJob {
    /// Draw a batch of tiles of one page.
    Tiles {
        /// The ticket the result carries.
        ticket: Ticket,
        /// The tiles.
        batch: TileBatch,
    },
    /// Draw a whole page at a resolution.
    Page {
        /// The ticket the result carries.
        ticket: Ticket,
        /// The page.
        page: PageIndex,
        /// The resolution.
        dpi: Dpi,
    },
    /// Search the whole document.
    Search {
        /// The ticket the result carries.
        ticket: Ticket,
        /// What to look for.
        query: SearchQuery,
    },
    /// Write a page selection as text.
    Text {
        /// The ticket the result carries.
        ticket: Ticket,
        /// The pages.
        pages: PageSelection,
        /// Plain text or Markdown.
        flavour: TextFlavour,
    },
    /// Write a page selection as a PDF.
    WritePdf {
        /// The ticket the result carries.
        ticket: Ticket,
        /// The pages.
        pages: PageSelection,
    },
    /// Apply page edits and write the new file.
    Edit {
        /// The ticket the result carries.
        ticket: Ticket,
        /// The edits, in order.
        ops: Vec<PageOp>,
    },
}

/// What a job produced, under the ticket it was given.
#[derive(Debug)]
pub enum PdfDone {
    /// The tiles drawn, and how the batch ended; a stopped or failed batch keeps what it drew.
    Tiles {
        /// The job's ticket.
        ticket: Ticket,
        /// The tiles, in the batch's order.
        tiles: Vec<Tile>,
        /// How the batch ended.
        end: End,
    },
    /// A page drawn.
    Page {
        /// The job's ticket.
        ticket: Ticket,
        /// The page, or why not.
        raster: Result<Raster, PdfError>,
    },
    /// A search finished; a stopped one keeps the hits of the pages it searched.
    Searched {
        /// The job's ticket.
        ticket: Ticket,
        /// The hits.
        hits: Hits,
        /// How the search ended.
        end: End,
    },
    /// Text written.
    Text {
        /// The job's ticket.
        ticket: Ticket,
        /// The text, or why not.
        text: Result<String, PdfError>,
    },
    /// A PDF or an edited PDF written, as the bytes of a whole file.
    Written {
        /// The job's ticket.
        ticket: Ticket,
        /// The file, or why not.
        file: Result<Vec<u8>, PdfError>,
    },
}

impl PdfDone {
    /// The ticket of the job this answers.
    pub fn ticket(&self) -> Ticket {
        match self {
            PdfDone::Tiles { ticket, .. }
            | PdfDone::Page { ticket, .. }
            | PdfDone::Searched { ticket, .. }
            | PdfDone::Text { ticket, .. }
            | PdfDone::Written { ticket, .. } => *ticket,
        }
    }
}

/// The PDF back end. A type with no state of its own: the document is shared, the worker holds the
/// caches, the job says what to do.
#[derive(Debug, Clone, Copy)]
pub struct PdfBackend;

impl Backend for PdfBackend {
    type Doc = PdfDocument;
    type Worker = PdfWorker;
    type Job = PdfJob;
    type Done = PdfDone;

    /// Runs `job` on the calling thread. A job that is not run to the end because `stop` was raised
    /// or its deadline passed keeps what it did (tiles, hits) or answers [`PdfError::Stopped`].
    fn run(doc: &PdfDocument, worker: &mut PdfWorker, job: PdfJob, stop: &Stop) -> PdfDone {
        match job {
            PdfJob::Tiles { ticket, batch } => {
                let (tiles, end) = render_tiles(doc, worker, &batch, stop);
                PdfDone::Tiles { ticket, tiles, end }
            }
            PdfJob::Page { ticket, page, dpi } => PdfDone::Page {
                ticket,
                raster: render_page(doc, worker, page, dpi, stop),
            },
            PdfJob::Search { ticket, query } => {
                let (hits, end) = search_document(doc, worker, &query, stop);
                PdfDone::Searched { ticket, hits, end }
            }
            PdfJob::Text {
                ticket,
                pages,
                flavour,
            } => PdfDone::Text {
                ticket,
                text: unless_stopped(stop, || write_text(doc, pages, flavour)),
            },
            PdfJob::WritePdf { ticket, pages } => PdfDone::Written {
                ticket,
                file: unless_stopped(stop, || write_pages(doc, pages)),
            },
            PdfJob::Edit { ticket, ops } => PdfDone::Written {
                ticket,
                file: unless_stopped(stop, || apply(doc, &ops)),
            },
        }
    }
}

/// Work that cannot be interrupted partway is not started once the stop is raised.
fn unless_stopped<T>(
    stop: &Stop,
    work: impl FnOnce() -> Result<T, PdfError>,
) -> Result<T, PdfError> {
    if Halt::new(stop).is_up() {
        Err(PdfError::Stopped)
    } else {
        work()
    }
}
