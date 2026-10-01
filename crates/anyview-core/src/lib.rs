//! The viewer's pure vocabulary: what a file is, how it is told apart, the units the viewer counts
//! in, the sequence arrow keys walk, the actions, edits and exports a file offers, and the view
//! memory a file keeps. No I/O, no clock, no renderer: effects belong to the crates above.
//!
//! Every public item is reached from this root, once.

mod action;
mod edit;
mod error;
mod facts;
mod kind;
mod peek;
mod profile;
mod sequence;
mod sniff;
mod source;
mod units;

pub use action::{FileAction, Reach, reach, shortcut};
pub use edit::{Edit, EditKind};
pub use error::CoreError;
pub use facts::{Fact, FactLabel, FactValue, Facts};
pub use kind::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, FormatDetail, FormatKind, MediaContainer,
    Mime, OfficeFormat, RasterFormat, SyntaxName, TextEncoding, TreeFormat,
};
pub use peek::{Peek, PeekBudget, StageSupport};
pub use profile::{actions_for, edits_for, stage_support};
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
    PageRange, PageSelection, Percent, Permille, PixelArea, PixelLen, PixelSize, Quality,
    QuarterTurn, Volume, Zoom,
};
