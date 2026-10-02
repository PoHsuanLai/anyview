//! The blocking work of the PDF stage and its answers. A task is what the window hands a worker:
//! the shared document, the window's `Gpu` and one thing to do. A worker draws a tile, a thumbnail
//! or a search with `anyview-pdf`'s back end and uploads pixels into a `TextureHandle` itself, so a
//! page's pixels never pass through the UI thread: the answer carries the handle, nothing else.

use super::doc::PdfDoc;
use crate::io::{Stop, WorkLane};
use crate::{Ticket, TypedText};
use anyview_core::{Dpi, PageIndex, PixelSize};
use anyview_pdf::{
    Backend, End, Hits, PdfBackend, PdfDone, PdfJob, PdfLink, Priority, Raster, SearchQuery,
    TileBatch, TileKey, page_links,
};
use ds_blitz::{Gpu, PixelFormat, Pixels, TextureHandle};
use std::sync::Arc;

/// The resolution of a thumbnail: small enough to draw in a blink, large enough for a panel's
/// column on a dense screen.
const THUMBNAIL: u32 = 36;

/// Which batch of tiles an answer settles. The window numbers them as it asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlightId(pub u64);

/// One thing a worker is asked to do for a PDF.
#[derive(Debug, Clone)]
pub enum PdfAsk {
    /// Draw a batch of tiles of one page and upload each.
    Tiles {
        /// The batch's number.
        flight: FlightId,
        /// What to draw, and how soon it is wanted.
        batch: TileBatch,
        /// Raised when the window no longer wants the batch.
        stop: Stop,
    },
    /// Find text in the whole document.
    Search {
        /// What to look for.
        query: TypedText,
        /// Raised when another search replaces this one.
        stop: Stop,
    },
    /// Draw one page small and upload it.
    Thumb {
        /// The page.
        page: PageIndex,
    },
    /// Read the links of one page.
    Links {
        /// The page.
        page: PageIndex,
    },
}

/// A task for a worker: a PDF job with what it needs to run and answer.
#[derive(Debug, Clone)]
pub struct PdfTask {
    ticket: Ticket,
    doc: Arc<PdfDoc>,
    gpu: Gpu,
    ask: PdfAsk,
}

/// A tile drawn and uploaded.
#[derive(Debug, Clone)]
pub struct ReadyTile {
    /// Which tile.
    pub key: TileKey,
    /// Where its pixels are.
    pub texture: TextureHandle,
    /// Its size in pixels at its zoom.
    pub size: PixelSize,
}

/// What a worker made of a [`PdfTask`].
#[derive(Debug)]
pub enum PdfAnswer {
    /// A batch of tiles: those drawn, those cut short (to ask again), those that cannot be drawn.
    Tiles {
        /// The batch's number.
        flight: FlightId,
        /// The tiles on the GPU.
        ready: Vec<ReadyTile>,
        /// The tiles not reached: the stop was raised, or the GPU was not there yet.
        unfinished: Vec<TileKey>,
        /// The tiles that failed to draw or upload.
        failed: Vec<TileKey>,
    },
    /// A search ended.
    Searched {
        /// The text searched for.
        query: TypedText,
        /// The hits of the pages searched.
        hits: Hits,
        /// Whether every page was searched.
        end: Finish,
    },
    /// A page drawn small; `None` when it could not be.
    Thumb {
        /// The page.
        page: PageIndex,
        /// Its pixels.
        texture: Option<TextureHandle>,
    },
    /// The links of a page.
    Links {
        /// The page.
        page: PageIndex,
        /// Where they are and where they lead.
        links: Vec<PdfLink>,
    },
}

/// How a search ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// Every page was searched.
    Complete,
    /// It was cut short; its hits are partial.
    Cut,
}

impl PdfTask {
    /// `ask` of the document of load `ticket`, uploading to `gpu`.
    pub fn new(ticket: Ticket, doc: Arc<PdfDoc>, gpu: Gpu, ask: PdfAsk) -> PdfTask {
        PdfTask {
            ticket,
            doc,
            gpu,
            ask,
        }
    }

    /// How soon the task is wanted: tiles of the view and a search are, tiles beyond it,
    /// thumbnails and links are not.
    pub fn lane(&self) -> WorkLane {
        match &self.ask {
            PdfAsk::Tiles { batch, .. } => match batch.priority {
                Priority::Visible => WorkLane::Visible,
                Priority::Preload => WorkLane::Preload,
            },
            PdfAsk::Search { .. } => WorkLane::Visible,
            PdfAsk::Thumb { .. } | PdfAsk::Links { .. } => WorkLane::Preload,
        }
    }

    /// The load the task belongs to.
    pub fn ticket(&self) -> Ticket {
        self.ticket
    }

    /// Do the task, blocking.
    pub fn run(self) -> PdfAnswer {
        let PdfTask {
            ticket,
            doc,
            gpu,
            ask,
        } = self;
        match ask {
            PdfAsk::Tiles {
                flight,
                batch,
                stop,
            } => tiles(ticket, &doc, &gpu, flight, batch, &stop),
            PdfAsk::Search { query, stop } => search(ticket, &doc, query, &stop),
            PdfAsk::Thumb { page } => PdfAnswer::Thumb {
                page,
                texture: thumb(ticket, &doc, &gpu, page),
            },
            PdfAsk::Links { page } => PdfAnswer::Links {
                page,
                links: page_links(&doc.document, page).unwrap_or_default(),
            },
        }
    }
}

/// What putting a drawn page on the GPU came to.
enum Upload {
    Done(TextureHandle),
    /// The window has no device yet: ask again once it has.
    Later,
    /// The pixels cannot be a texture.
    Never,
}

fn upload(gpu: &Gpu, raster: &Raster) -> Upload {
    let size = raster.size();
    let Ok(pixels) = Pixels::new(
        PixelFormat::Rgba8Premultiplied,
        size.width.0,
        size.height.0,
        raster.premultiplied_rgba(),
    ) else {
        return Upload::Never;
    };
    match gpu.upload(&pixels) {
        Ok(texture) => Upload::Done(texture),
        Err(ds_blitz::GpuError::NotReady) => Upload::Later,
        Err(ds_blitz::GpuError::TooLarge { max: _ }) => Upload::Never,
    }
}

fn tiles(
    ticket: Ticket,
    doc: &PdfDoc,
    gpu: &Gpu,
    flight: FlightId,
    batch: TileBatch,
    stop: &Stop,
) -> PdfAnswer {
    let keys: Vec<TileKey> = batch
        .tiles
        .iter()
        .map(|at| TileKey {
            page: batch.page,
            zoom: batch.zoom,
            x: at.x,
            y: at.y,
        })
        .collect();
    let job = PdfJob::Tiles { ticket, batch };
    let done = doc.with_scratch(|worker| PdfBackend::run(&doc.document, worker, job, stop));
    let (drawn, end) = match done {
        PdfDone::Tiles { tiles, end, .. } => (tiles, end),
        PdfDone::Page { .. }
        | PdfDone::Searched { .. }
        | PdfDone::Text { .. }
        | PdfDone::Written { .. } => (Vec::new(), End::Complete),
    };
    let (mut ready, mut unfinished, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for tile in drawn {
        match upload(gpu, &tile.raster) {
            Upload::Done(texture) => ready.push(ReadyTile {
                key: tile.key,
                texture,
                size: tile.raster.size(),
            }),
            Upload::Later => unfinished.push(tile.key),
            Upload::Never => failed.push(tile.key),
        }
    }
    let missing = keys
        .into_iter()
        .filter(|key| !ready.iter().any(|tile| tile.key == *key))
        .filter(|key| !unfinished.contains(key) && !failed.contains(key));
    let mut missing: Vec<TileKey> = missing.collect();
    // The tile a failed draw stopped at cannot be drawn; the ones after it were never tried.
    if let End::Failed(_) = end
        && !missing.is_empty()
    {
        failed.push(missing.remove(0));
    }
    unfinished.extend(missing);
    PdfAnswer::Tiles {
        flight,
        ready,
        unfinished,
        failed,
    }
}

fn search(ticket: Ticket, doc: &PdfDoc, query: TypedText, stop: &Stop) -> PdfAnswer {
    let job = PdfJob::Search {
        ticket,
        query: SearchQuery::new(query.as_str()),
    };
    let done = doc.with_scratch(|worker| PdfBackend::run(&doc.document, worker, job, stop));
    let (hits, end) = match done {
        PdfDone::Searched { hits, end, .. } => (hits, finish(&end)),
        PdfDone::Tiles { .. }
        | PdfDone::Page { .. }
        | PdfDone::Text { .. }
        | PdfDone::Written { .. } => (Hits::default(), Finish::Cut),
    };
    PdfAnswer::Searched { query, hits, end }
}

fn finish(end: &End) -> Finish {
    match end {
        End::Complete => Finish::Complete,
        End::Stopped | End::Failed(_) => Finish::Cut,
    }
}

fn thumb(ticket: Ticket, doc: &PdfDoc, gpu: &Gpu, page: PageIndex) -> Option<TextureHandle> {
    let dpi = Dpi::new(THUMBNAIL).ok()?;
    let job = PdfJob::Page { ticket, page, dpi };
    let done = doc.with_scratch(|worker| PdfBackend::run(&doc.document, worker, job, &Stop::new()));
    let PdfDone::Page { raster, .. } = done else {
        return None;
    };
    match upload(gpu, &raster.ok()?) {
        Upload::Done(texture) => Some(texture),
        Upload::Later | Upload::Never => None,
    }
}
