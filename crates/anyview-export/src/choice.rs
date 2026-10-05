//! What a person chose to export of a file that is not a recording.

use anyview_core::{PdfExport, RasterExport, TextExport};

/// An export of an image, a PDF or a text document: the three formats this crate writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentExport {
    /// An image, or an SVG.
    Raster(RasterExport),
    /// A PDF.
    Pdf(PdfExport),
    /// Markdown, source code or plain text.
    Text(TextExport),
}
