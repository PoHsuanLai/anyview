//! What a PDF can be exported as.

use super::choice::ExportChoice;
use super::extension::ExportExtension;
use super::target::RasterTarget;
use crate::units::{Dpi, PageSelection};
use ds_core::word::Word;

/// An export of a PDF document. Page choices are a [`PageSelection`], not a range, because "all
/// pages" is the default and is picked before the document's length is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PdfExport {
    /// A PDF of some of the pages.
    Pdf(PageSelection),
    /// One image per page, rendered at a resolution.
    PageImages(PageSelection, RasterTarget, Dpi),
    /// The text of the document.
    PlainText,
    /// The text with its structure as Markdown.
    Markdown,
}

/// The rows of the PDF export dialog's format list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PdfExportKind {
    /// A PDF of some pages.
    Pdf,
    /// Images of the pages.
    PageImages,
    /// Plain text.
    PlainText,
    /// Markdown.
    Markdown,
}

impl ExportChoice for PdfExport {
    type Kind = PdfExportKind;

    fn kind(&self) -> PdfExportKind {
        match self {
            PdfExport::Pdf(_) => PdfExportKind::Pdf,
            PdfExport::PageImages(..) => PdfExportKind::PageImages,
            PdfExport::PlainText => PdfExportKind::PlainText,
            PdfExport::Markdown => PdfExportKind::Markdown,
        }
    }

    fn default_for(kind: PdfExportKind) -> Self {
        match kind {
            PdfExportKind::Pdf => PdfExport::Pdf(PageSelection::All),
            PdfExportKind::PageImages => {
                PdfExport::PageImages(PageSelection::All, RasterTarget::Png, Dpi::SCREEN)
            }
            PdfExportKind::PlainText => PdfExport::PlainText,
            PdfExportKind::Markdown => PdfExport::Markdown,
        }
    }

    fn extension(&self) -> ExportExtension {
        match self {
            PdfExport::Pdf(_) => ExportExtension::Pdf,
            PdfExport::PageImages(_, target, _) => target.extension(),
            PdfExport::PlainText => ExportExtension::Txt,
            PdfExport::Markdown => ExportExtension::Md,
        }
    }
}
