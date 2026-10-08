//! PDF for the viewer: open a document once and share it, size and lay out its pages, plan the
//! tiles a view needs and draw them, search across the document, read its outline and links, edit
//! its pages and write exports.
//!
//! Everything blocks and runs on the caller's worker: the crate spawns nothing, and the binary's
//! pool calls [`PdfBackend::run`]. Pixels come out as CPU buffers ([`Raster`]); uploading them to a
//! texture belongs to the view. Page images are encoded by `anyview-image`, not here.

mod bind;
mod document;
mod edit;
mod error;
mod export;
mod geometry;
mod halt;
mod job;
mod layout;
mod outline;
mod pictures;
mod render;
mod search;
mod tile;

pub use anyview_core::work::{Backend, Stop, StopState, Ticket};
pub use bind::{Bookmark, bind};
pub use document::{DocId, PdfDocument};
pub use edit::{PageOp, apply, page_op};
pub use error::PdfError;
pub use export::{plan_export, selected, write_pages, write_text};
pub use geometry::{MilliPoints, PageRect, PageSize, displayed};
pub use job::{PdfBackend, PdfDone, PdfJob};
pub use layout::{PageLayout, PagePlace};
pub use outline::{Disclosure, LinkTarget, OutlineEntry, PdfLink, outline, page_links};
pub use pictures::{PagePicture, pdf_of_pictures};
pub use render::{End, PdfWorker, Raster, Tile};
pub use search::{CaseMatch, Hit, Hits, SearchQuery, WordMatch, search_document, search_page};
pub use tile::{
    Priority, Schedule, TILE_SIDE, TileBatch, TileCoord, TileKey, TileRect, ViewWindow, ZoomBucket,
    device_bound, schedule, tile_grid, tile_rect,
};
