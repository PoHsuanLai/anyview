//! What Markdown, code and plain text can be exported as.

use super::choice::ExportChoice;
use super::extension::ExportExtension;
use super::layout::PrintLayout;
use ds_core::word::Word;

/// An export of a text document: Markdown, source code or plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextExport {
    /// A PDF with real, selectable text, laid out on a page.
    Pdf(PrintLayout),
    /// The text as it is.
    PlainText,
}

/// The rows of the text export dialog's format list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TextExportKind {
    /// A PDF.
    Pdf,
    /// Plain text.
    PlainText,
}

impl ExportChoice for TextExport {
    type Kind = TextExportKind;

    fn kind(&self) -> TextExportKind {
        match self {
            TextExport::Pdf(_) => TextExportKind::Pdf,
            TextExport::PlainText => TextExportKind::PlainText,
        }
    }

    fn default_for(kind: TextExportKind) -> Self {
        match kind {
            TextExportKind::Pdf => TextExport::Pdf(PrintLayout::default()),
            TextExportKind::PlainText => TextExport::PlainText,
        }
    }

    fn extension(&self) -> ExportExtension {
        match self {
            TextExport::Pdf(_) => ExportExtension::Pdf,
            TextExport::PlainText => ExportExtension::Txt,
        }
    }
}
