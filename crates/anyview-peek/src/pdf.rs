//! The peek of a PDF: its first page. With the `pane` feature it is rasterised by `ds-blitz`'s
//! thumbnail cache (see `raster`); without it a PDF is peeked as facts only, since a headless
//! build links no renderer.

use ds::components::content::pdf_thumb::PdfPage;

#[cfg(feature = "pane")]
mod raster;

#[cfg(feature = "pane")]
pub use raster::PdfPeek;

/// What a peek of a PDF holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfPeeked {
    /// The first page, rasterised, or why it could not be drawn: the pane's `PdfThumb` draws each.
    pub page: PdfPage,
}

/// The kind `Pdf` as a file with no page to draw.
#[cfg(not(feature = "pane"))]
#[derive(Debug, Clone, Copy)]
pub struct PdfKind;

#[cfg(not(feature = "pane"))]
impl crate::described::Describes for PdfKind {
    const KIND: anyview_core::FormatKind = anyview_core::FormatKind::Pdf;
}

/// The facts-only peek of a PDF.
#[cfg(not(feature = "pane"))]
pub type PdfPeek = crate::described::FactsPeek<PdfKind>;
