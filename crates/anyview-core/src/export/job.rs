//! `ExportJob`: the shared encoders every format's export turns into.

use super::layout::PrintLayout;
use super::payload::{HtmlDoc, MetadataCarry, PdfPages, PixelSource, Subtitles, TextSource};
use super::target::RasterTarget;

/// One unit of export work, as data. A format turns the person's choice into jobs and the back
/// ends only ever see this enum, so adding a format adds no case here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportJob {
    /// Encode pixels as a raster image.
    EncodeRaster {
        /// Where the pixels come from.
        pixels: PixelSource,
        /// The encoding.
        target: RasterTarget,
        /// Whether to keep the original's metadata.
        keep: MetadataCarry,
    },
    /// Write a PDF from existing pages or from images placed as pages.
    WritePdf {
        /// The pages.
        pages: PdfPages,
    },
    /// Print an HTML rendering of a document to a vector PDF with real text.
    PrintToPdf {
        /// The document.
        html: HtmlDoc,
        /// The page.
        layout: PrintLayout,
    },
    /// Write text to a file.
    WriteText {
        /// The text.
        text: TextSource,
    },
    /// Ask the media player for a full-resolution frame.
    MpvScreenshot {
        /// The encoding.
        target: RasterTarget,
        /// Whether subtitles are drawn in.
        subtitles: Subtitles,
    },
}
