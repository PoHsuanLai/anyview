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
mod frames;
mod media;
mod office;
mod pane;
mod pdf;
mod probe;
mod registry;
mod when;

pub use any::{AnyPeeked, peek, peek_with};
pub use body::{Body, Light};
pub use described::{BookKind, BookPeek, Described, Describes, FactsPeek, OtherKind, OtherPeek};
pub use error::PeekError;
pub use folder::{FOLDER_ENTRIES, FolderPeek, FolderSummary, KindCount};
pub use frames::{NoFrames, VideoFrames};
pub use media::{AudioPeek, MediaLook, VideoPeek};
pub use office::{OfficeLooked, OfficePeek};
pub use pane::{Pane, STYLE};
pub use pdf::{PdfPeek, PdfPeeked};
pub use probe::{Probed, probe};
pub use registry::{KindVisitor, visit};
pub use when::modified_text;
