//! The viewer's pure vocabulary: what a file is, how it is told apart, the units the viewer counts
//! in, the sequence arrow keys walk, the actions, edits and exports a file offers, and the view
//! memory a file keeps. No I/O, no clock, no renderer: effects belong to the crates above.
//!
//! Every public item is reached from this root, once, except the contract with the threads, which
//! is reached through its one public module, `work`, and the front doors, which [`prelude`] names
//! a second time for a glob import.

mod action;
mod edit;
mod error;
mod export;
mod facts;
mod helper;
mod kind;
mod media;
mod peek;
pub mod prelude;
mod profile;
mod resume;
mod sequence;
mod sniff;
mod source;
mod trail;
mod tree_path;
mod units;
pub mod work;
mod xml_depth;

/// What a person sees the program called: the window titles, the now-playing entry and the helper sheet.
/// The binary, the app id and the bus names stay `anyview`, `org.quire.Anyview` and `org.quire.Anyview1`.
pub const APP_NAME: &str = "Viewer";

pub use action::{FileAction, Reach, reach, shortcut};
pub use edit::{Edit, EditKind};
pub use error::CoreError;
pub use export::{
    AudioTarget, ExportChoice, ExportExtension, ExportJob, HtmlDoc, MediaExport, MediaExportKind,
    MetadataCarry, NoExport, NoExportKind, Orientation, PaperSize, PdfExport, PdfExportKind,
    PdfPages, PixelSource, PrintLayout, RasterExport, RasterExportKind, RasterTarget, Resize,
    StreamPick, Subtitles, TextExport, TextExportKind, TextFlavour, TextSource,
};
pub use facts::{
    Coordinate, Fact, FactGroup, FactLabel, FactTime, FactValue, FactZone, Facts, FileDetails,
    LocalZone, Tier, kind_name,
};
pub use helper::Helper;
pub use kind::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, FormatDetail, FormatKind, MediaContainer,
    Mime, OfficeFormat, RasterFormat, SyntaxName, TextEncoding, TreeFormat, claimed_mimes,
    kind_of_mime, opened_globs, opened_mimes,
};
pub use media::{MediaChapter, MediaTags, MediaTrack, StreamKind, TrackPlay, VideoPresence};
pub use peek::{Deadline, Peek, PeekBudget, StageSupport};
pub use profile::{actions_for, edits_for, mime_for, stage_support};
pub use resume::{Resume, TrackChoice, TrackId};
pub use sequence::{
    Heading, Neighbours, NonEmpty, ResultsId, Sequence, SequenceMove, SequenceOrigin,
    SequencePosition, moved, neighbours, without_current,
};
pub use sniff::{
    FileHead, SniffStep, Sniffed, ZipEntries, ZipProbe, sniff, sniff_folder, sniff_zip,
};
pub use source::{
    ByteLen, FileName, FilePath, FileStamp, Input, ModTime, ReadAt, ReadAtStream, Source,
};
pub use trail::{Trail, TrailIn, TrailOut, TrailStacks};
pub use tree_path::{OpenNodes, TreePath};
pub use units::{
    Axis, Bitrate, ChapterIndex, DocPoint, DocUnit, Dpi, LineIndex, MediaLength, MediaTime,
    PageCount, PageIndex, PageRange, PageSelection, Percent, Permille, PixelArea, PixelLen,
    PixelSize, Quality, QuarterTurn, SectionCount, SectionIndex, Speed, TimeRange, Volume, Zoom,
};
pub use xml_depth::{MAX_XML_DEPTH, nests_deeper_than};
