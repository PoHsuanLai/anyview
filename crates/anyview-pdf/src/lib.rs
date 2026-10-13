//! PDF for the viewer: open a document once and share it, size and lay out its pages, plan the
//! tiles a view needs and draw them, search across the document, read its outline and links, edit
//! its pages and write exports.
//!
//! Everything blocks and runs on the caller's worker: the crate spawns nothing, and the binary's
//! pool calls [`PdfBackend::run`]. Pixels come out as CPU buffers ([`Raster`]); uploading them to a
//! texture belongs to the view. Page images are encoded by `anyview-image`, not here.
//!
//! The front door is [`PdfDocument`]: open the file once, then ask it for its pages.
//!
//! ```
//! use anyview_core::{PageIndex, PixelLen, PixelSize};
//! use anyview_pdf::{PagePicture, PdfDocument, pdf_of_pictures};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Two pages made in memory, each a grey picture the size of its page.
//! let page = |width: u32, height: u32| PagePicture::Pixels {
//!     size: PixelSize {
//!         width: PixelLen(width),
//!         height: PixelLen(height),
//!     },
//!     rgba: [128u8, 128, 128, 255]
//!         .iter()
//!         .copied()
//!         .cycle()
//!         .take(width as usize * height as usize * 4)
//!         .collect(),
//! };
//! let bytes = pdf_of_pictures(&[page(80, 40), page(40, 80)])?;
//!
//! // Opened once, the document knows its pages and their sizes.
//! let document = PdfDocument::from_bytes(bytes)?;
//! assert_eq!(document.page_count().get(), 2);
//! let first = document.page_size(PageIndex(0))?;
//! let second = document.page_size(PageIndex(1))?;
//! assert!(first.width > first.height);
//! assert!(second.width < second.height);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod bind;
mod document;
mod edit;
mod error;
mod export;
mod geometry;
mod halt;
mod info;
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
pub use info::{FormatVersion, Orientation, PageFormat, PdfInfo, Protection, Restriction, Tagging};
pub use job::{PdfBackend, PdfDone, PdfJob};
pub use layout::{PageLayout, PagePlace};
pub use outline::{Disclosure, LinkTarget, OutlineEntry, PdfLink, outline, page_links};
pub use pictures::{PagePicture, pdf_of_pictures};
pub use render::{End, PdfWorker, Raster, Tile};
pub use search::{Hit, Hits, SearchQuery, Snippet, search_document, search_page};
pub use tile::{
    Priority, Schedule, TILE_SIDE, TileBatch, TileCoord, TileKey, TileRect, ViewWindow, ZoomBucket,
    device_bound, schedule, tile_grid, tile_rect,
};
