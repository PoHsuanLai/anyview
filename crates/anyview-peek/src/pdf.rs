//! The peek of a PDF: its first page. With the `pane` feature it is rasterised by `ds-blitz`'s
//! thumbnail cache (see `raster`); without it a PDF is peeked as facts only, since a headless
//! build links no renderer.

use ds_core::word::Word;

#[cfg(feature = "pane")]
pub(crate) mod raster;

#[cfg(feature = "pane")]
pub use raster::PdfPeek;

/// What a peek of a PDF holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfPeeked {
    /// The first page, rasterised, or why it could not be drawn: the pane draws each.
    pub page: PageLook,
}

/// What a peek of a PDF's first page came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageLook {
    /// The first page, rasterised.
    Drawn {
        /// The page's pixels as a `data:` URI or a `file:` URL, ready to be an `<img>`'s `src`.
        source: String,
        /// The page's displayed width in points, after rotation; only its ratio is read.
        width: u32,
        /// The page's displayed height in points, after rotation.
        height: u32,
    },
    /// A document with no pages.
    Blank,
    /// The page could not be drawn.
    Failed(PageTrouble),
}

/// Why a PDF's first page could not be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PageTrouble {
    /// Not a PDF, damaged past recovery, unreadable, or a page the rasteriser refused.
    #[word(label = "PDF, no preview")]
    Unreadable,
    /// Encrypted with a password that is not the empty one.
    #[word(label = "Locked PDF")]
    Locked,
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
