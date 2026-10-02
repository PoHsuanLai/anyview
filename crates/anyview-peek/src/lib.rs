//! The viewer's light tier: one peek per kind of file, the registry that maps every kind to its
//! peek, and the pane that draws what was peeked at. This is what the launcher links, so it holds
//! no media player, no D-Bus and no windowing of its own.
//!
//! Every public item is reached from this root, once.

mod any;
mod body;
mod described;
mod error;
mod folder;
mod pdf;
mod registry;
mod when;

pub use any::{AnyPeeked, peek};
pub use body::{Body, Light};
pub use described::{
    ArchiveKind, ArchivePeek, AudioKind, AudioPeek, BookKind, BookPeek, Described, Describes,
    FactsPeek, FontKind, FontPeek, OfficeKind, OfficePeek, OtherKind, OtherPeek, VideoKind,
    VideoPeek,
};
pub use error::PeekError;
pub use folder::{FOLDER_ENTRIES, FolderPeek, FolderSummary, KindCount};
pub use pdf::{PdfPeek, PdfPeeked};
pub use registry::{KindVisitor, visit};
pub use when::modified_text;
