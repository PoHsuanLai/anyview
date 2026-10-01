//! The viewer's pure vocabulary.

mod error;
mod kind;
mod sequence;
mod sniff;
mod source;
mod units;

pub use error::CoreError;
pub use kind::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, FormatDetail, FormatKind, MediaContainer,
    Mime, OfficeFormat, RasterFormat, SyntaxName, TextEncoding, TreeFormat,
};
pub use sequence::{
    Neighbours, NonEmpty, ResultsId, Sequence, SequenceMove, SequenceOrigin, SequencePosition,
    moved, neighbours,
};
pub use sniff::{
    FileHead, SniffStep, Sniffed, ZipEntries, ZipProbe, sniff, sniff_folder, sniff_zip,
};
pub use source::{ByteLen, FileName, FilePath, FileStamp, ModTime, Source};
pub use units::{
    Axis, DocPoint, DocUnit, Dpi, LineIndex, MediaLength, MediaTime, PageCount, PageIndex,
    PageRange, PageSelection, Percent, Permille, PixelArea, PixelLen, PixelSize, QuarterTurn,
    Quality, Volume, Zoom,
};
