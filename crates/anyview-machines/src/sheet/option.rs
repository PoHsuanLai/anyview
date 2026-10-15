//! The options of an export and how the sheet's machine applies one to a draft. Every option
//! checks itself against the draft and the open file: one that means nothing for this draft is
//! dropped, one outside its limits is brought inside.

use anyview_core::{
    Bitrate, Dpi, MetadataCarry, Orientation, PaperSize, Percent, Permille, PixelLen, Quality,
    RasterExportKind, Resize,
};

/// The longest side an exported picture may be asked for, in pixels.
pub const MAX_LONG_EDGE: PixelLen = PixelLen(16_384);

/// Which pages the sheet's page choice stands for. It is the sheet's own state because a range
/// that happens to be one page is still a range the person is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageSpan {
    /// Every page.
    #[default]
    All,
    /// The page the reader is on.
    Current,
    /// A run of pages, set with its first and last.
    Between,
}

/// Which part of a recording a trim keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrimSpan {
    /// The whole recording.
    Whole,
    /// What lies between the marks.
    Marks,
}

/// How a picture's size is said in the dialog: the sizes on offer, and the long edge a stepper sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizePick {
    /// As it is.
    Original,
    /// Half each way.
    Half,
    /// A quarter each way.
    Quarter,
    /// A long edge in pixels.
    LongEdge,
}

impl SizePick {
    /// The scale of "50%", in thousandths.
    const HALF: Permille = Permille(500);
    /// The scale of "25%", in thousandths.
    const QUARTER: Permille = Permille(250);

    /// The segment `resize` is, or `None` for a scale that is neither half nor a quarter: no
    /// segment is lit for a size the person did not pick from them.
    #[must_use]
    pub fn of(resize: Resize) -> Option<SizePick> {
        match resize {
            Resize::Original => Some(SizePick::Original),
            Resize::Scaled(scale) if scale == SizePick::HALF => Some(SizePick::Half),
            Resize::Scaled(scale) if scale == SizePick::QUARTER => Some(SizePick::Quarter),
            Resize::Scaled(_) => None,
            Resize::LongEdge(_) => Some(SizePick::LongEdge),
        }
    }

    /// The resize this segment asks for; `long` is the edge a long-edge pick starts on.
    #[must_use]
    pub fn resize(self, long: PixelLen) -> Resize {
        match self {
            SizePick::Original => Resize::Original,
            SizePick::Half => Resize::Scaled(SizePick::HALF),
            SizePick::Quarter => Resize::Scaled(SizePick::QUARTER),
            SizePick::LongEdge => Resize::LongEdge(long),
        }
    }
}

/// One option of an export changing. Page numbers count from one, as the person reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportOption {
    /// The quality of a JPEG or an AVIF.
    Quality(Percent),
    /// How a picture is resized.
    Resize(Resize),
    /// How much of a picture's metadata it keeps.
    Metadata(MetadataCarry),
    /// Which pages: all, the current one, or a range.
    Pages(PageSpan),
    /// The first page of a range, from 1.
    PagesFrom(u32),
    /// The last page of a range, from 1.
    PagesTo(u32),
    /// The format of the page images of a PDF.
    ImageFormat(RasterExportKind),
    /// The resolution of the page images of a PDF.
    Resolution(Dpi),
    /// The paper a text is printed on.
    Paper(PaperSize),
    /// Which way up the paper is.
    Orientation(Orientation),
    /// The part of a recording a trim keeps.
    Trim(TrimSpan),
    /// The bitrate of lossy audio.
    Bitrate(Bitrate),
}

/// One control of the export sheet's options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportControl {
    /// The quality slider.
    Quality,
    /// The picture's size.
    Size,
    /// How much metadata is kept: all, all but the location, or none.
    Metadata,
    /// All, the current page or a range.
    Pages,
    /// The first and last page of a range.
    PageRange,
    /// The format of the page images.
    ImageFormat,
    /// The resolution of the page images.
    Resolution,
    /// The paper.
    Paper,
    /// Which way up the paper is.
    Orientation,
    /// The whole recording or what lies between the marks.
    Range,
    /// The bitrate.
    Bitrate,
}
