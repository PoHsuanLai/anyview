//! Drawing: a batch of tiles of one page, or a whole page at a resolution, into CPU pixels with
//! pdfrum's vello-cpu rasterizer. Blocking; the caller's worker runs it.

use crate::document::{DocId, PdfDocument};
use crate::error::PdfError;
use crate::halt::Halt;
use crate::tile::{TileBatch, TileKey, tile_rect};
use anyview_core::work::Stop;
use anyview_core::{Dpi, PageIndex, PixelLen, PixelSize};
use pdfrum::{Color, DeviceRect, Pixmap, Region, RenderOptions, RenderSession, VelloCpuBackend};

/// Scratch for one worker: pdfrum's caches of fonts, colour spaces and glyph outlines, which draw
/// the next tile faster than the first. They belong to one document, so the worker starts fresh
/// when it is given another. Keep one per worker thread; it is `Send`, not `Sync`.
#[derive(Debug, Default)]
pub struct PdfWorker {
    bound: Option<DocId>,
    session: RenderSession,
}

impl PdfWorker {
    /// A worker with empty caches.
    pub fn new() -> PdfWorker {
        PdfWorker::default()
    }

    pub(crate) fn session(&mut self, doc: &PdfDocument) -> &mut RenderSession {
        if self.bound != Some(doc.id()) {
            self.session = doc.inner().render_session();
            self.bound = Some(doc.id());
        }
        &mut self.session
    }
}

/// Pixels: 8-bit RGBA, **premultiplied** alpha (what the GPU compositor blends with), rows top to
/// bottom with no padding. A page is drawn on white, so every pixel is opaque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster(Pixmap);

impl Raster {
    /// The width and height.
    pub fn size(&self) -> PixelSize {
        PixelSize {
            width: PixelLen(self.0.width()),
            height: PixelLen(self.0.height()),
        }
    }

    /// The pixels, `R G B A` each, premultiplied.
    pub fn premultiplied_rgba(&self) -> &[u8] {
        self.0.data()
    }

    /// The pixels with straight alpha, for an encoder.
    pub fn straight_rgba(&self) -> Vec<u8> {
        self.0.to_straight_rgba()
    }
}

/// One drawn tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// Which tile it is.
    pub key: TileKey,
    /// Its pixels; edge tiles are smaller than [`TILE_SIDE`](crate::TILE_SIDE).
    pub raster: Raster,
}

/// How a job that does its work in steps ended.
#[derive(Debug)]
pub enum End {
    /// Every step was done.
    Complete,
    /// The stop was raised or its budget spent; the steps before it are kept.
    Stopped,
    /// A step failed; the steps before it are kept.
    Failed(PdfError),
}

impl End {
    pub(crate) fn of(error: pdfrum::Error, halt: &Halt<'_>) -> End {
        if halt.is_up() {
            End::Stopped
        } else {
            End::Failed(error.into())
        }
    }
}

fn options(scale: f64) -> RenderOptions {
    RenderOptions::builder()
        .scale(scale)
        .background(Color::WHITE)
        .build()
}

/// The page's size in pixels at `scale`, worked out as the rasterizer works it out (the float
/// product, truncated), because a tile must lie inside it.
fn device_size(page: &pdfrum::Page<'_>, scale: f64) -> PixelSize {
    PixelSize {
        width: PixelLen((page.width() * scale).trunc() as u32),
        height: PixelLen((page.height() * scale).trunc() as u32),
    }
}

/// Draws the tiles of `batch`, reading the page once. Tiles past the page's real edge are left
/// out. A raised stop ends the batch between tiles (and mid-tile, inside pdfrum), keeping the
/// tiles already drawn.
pub(crate) fn render_tiles(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    batch: &TileBatch,
    stop: &Stop,
) -> (Vec<Tile>, End) {
    let halt = Halt::new(stop);
    let session = worker.session(doc);
    session.set_deadline(Some(halt.deadline().clone()));
    let mut tiles = Vec::with_capacity(batch.tiles.len());
    let end = draw_tiles(doc, session, batch, &halt, &mut tiles);
    session.set_deadline(None);
    (tiles, end)
}

fn draw_tiles(
    doc: &PdfDocument,
    session: &mut RenderSession,
    batch: &TileBatch,
    halt: &Halt<'_>,
    tiles: &mut Vec<Tile>,
) -> End {
    if halt.is_up() {
        return End::Stopped;
    }
    let page = match doc.page(batch.page) {
        Ok(page) => page,
        Err(error) => return End::Failed(error),
    };
    let scale = f64::from(batch.zoom.scale().0) / 1000.0;
    let device = device_size(&page, scale);
    let prepared = page.prepare(&options(scale), session);
    for coord in &batch.tiles {
        if halt.is_up() {
            return End::Stopped;
        }
        let Some(rect) = tile_rect(device, coord.x, coord.y) else {
            continue;
        };
        let region = match DeviceRect::new(rect.x, rect.y, rect.width, rect.height) {
            Ok(region) => Region::Rect(region),
            Err(error) => return End::of(pdfrum::Error::Render(error), halt),
        };
        match prepared.render_region_on(VelloCpuBackend, session, region) {
            Ok(pixmap) => tiles.push(Tile {
                key: TileKey {
                    page: batch.page,
                    zoom: batch.zoom,
                    x: coord.x,
                    y: coord.y,
                },
                raster: Raster(pixmap),
            }),
            Err(error) => return End::of(error, halt),
        }
    }
    End::Complete
}

/// Draws all of `page` at `dpi` (72 dpi is a point to a pixel) for an export.
pub(crate) fn render_page(
    doc: &PdfDocument,
    worker: &mut PdfWorker,
    page: PageIndex,
    dpi: Dpi,
    stop: &Stop,
) -> Result<Raster, PdfError> {
    let halt = Halt::new(stop);
    let session = worker.session(doc);
    session.set_deadline(Some(halt.deadline().clone()));
    let scale = f64::from(dpi.get()) / 72.0;
    let drawn = doc
        .page(page)
        .and_then(|page| {
            page.render_on(VelloCpuBackend, &options(scale), session)
                .map_err(PdfError::from)
        })
        .map(Raster);
    session.set_deadline(None);
    drawn.map_err(|error| {
        if halt.is_up() && matches!(error, PdfError::Pdf(_)) {
            PdfError::Stopped
        } else {
            error
        }
    })
}
