//! The payloads of an export job, described as data. A back end reads the named files and does
//! the work; nothing here holds bytes.

use super::target::Resize;
use crate::kind::SyntaxName;
use crate::sequence::NonEmpty;
use crate::source::FilePath;
use crate::units::{Dpi, PageIndex, PageSelection};
use ds_core::word::Word;

/// The pixels an encode starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelSource {
    /// An image file, resized.
    Image {
        /// The file.
        file: FilePath,
        /// How to resize it before encoding.
        resize: Resize,
    },
    /// One page of a PDF, rendered.
    PdfPage {
        /// The file.
        file: FilePath,
        /// Which page.
        page: PageIndex,
        /// The resolution to render at.
        dpi: Dpi,
    },
}

/// The pages a PDF is written from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfPages {
    /// Pages of an existing PDF.
    Pages {
        /// The file.
        file: FilePath,
        /// Which pages.
        pages: PageSelection,
    },
    /// Image files, one placed on each page.
    Images(NonEmpty<FilePath>),
}

/// A document rendered to HTML before it is printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlDoc {
    /// A Markdown file.
    Markdown(FilePath),
    /// A source file, highlighted as this syntax.
    Code {
        /// The file.
        file: FilePath,
        /// How to highlight it.
        syntax: SyntaxName,
    },
    /// A plain text file.
    PlainText(FilePath),
}

/// What form text extracted from a PDF takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TextFlavour {
    /// Text with no structure.
    Plain,
    /// Text with headings and lists as Markdown.
    Markdown,
}

/// The text a text export writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSource {
    /// The text of a PDF.
    Pdf {
        /// The file.
        file: FilePath,
        /// Which pages.
        pages: PageSelection,
        /// Plain or Markdown.
        flavour: TextFlavour,
    },
    /// A text file, copied as it is.
    File(FilePath),
}

/// What of a picture's metadata survives a re-encode. The colour profile always does: without it a
/// wide-gamut photo looks washed out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum MetadataCarry {
    /// Splice all of the original's EXIF into the output.
    Keep,
    /// Splice the original's EXIF without where the picture was taken. The default: a file made to
    /// be shared must not say where its photo was shot unless the person chooses that.
    #[default]
    StripLocation,
    /// Write the output with no EXIF.
    Drop,
}

/// Whether subtitles are drawn into an exported frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum Subtitles {
    /// Draw the shown subtitles into the picture.
    Burn,
    /// Leave the picture clean.
    #[default]
    Omit,
}

/// Which streams of a recording a transcode keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum StreamPick {
    /// Every stream: the recording as it is, cut or not.
    #[default]
    Everything,
    /// The audio track only.
    AudioOnly,
}
