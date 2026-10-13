//! The viewer's light tier: one peek per kind of file, the registry that maps every kind to its
//! peek, and the card that draws what was peeked at. This is what the launcher links, so it holds
//! no media player, no D-Bus and no windowing of its own.
//!
//! The front door is [`look`]: it probes a file and peeks at it as [`Peeking`] says, and gives the
//! preview a card draws ([`AnyPeeked`]). A path is made an input with `anyview_fs::OnDisk::on_disk`
//! (this crate's callers own the I/O of a path; `anyview-core` does none), and bytes a host holds
//! are an input as they are. [`probe`] and [`peek`] are the two halves, for a caller that holds the
//! probe between them; [`PeekWorker`] runs `look` off the UI thread.
//!
//! Every public item is reached from this root, once, except that [`prelude`] names the front doors
//! a second time for a glob import.

#![warn(missing_docs)]

mod any;
mod body;
mod book;
#[cfg(feature = "pane")]
mod card;
mod described;
mod error;
mod folder;
mod frames;
mod looking;
mod media;
mod natural;
mod office;
mod pdf;
pub mod prelude;
mod probe;
mod registry;
mod unavailable;
mod when;
mod worker;

pub use any::{AnyPeeked, peek};
pub use body::{Body, Light};
pub use book::{BookLook, BookPeek};
#[cfg(feature = "pane")]
pub use card::{Card, Part, Parts, STYLE};
pub use described::{Described, Describes, FactsPeek, OtherKind, OtherPeek};
pub use error::PeekError;
pub use folder::{FOLDER_ENTRIES, FolderPeek, FolderSummary, KindCount};
pub use frames::{NoStills, StillSource};
pub use looking::{Peeking, look};
pub use media::{AudioCover, AudioPeek, MediaLook, VideoPeek, audio_cover};
pub use natural::{is_audio, natural_size};
pub use office::{OfficeLooked, OfficePeek};
pub use pdf::{PageLook, PageTrouble, PdfPeek, PdfPeeked};
pub use probe::{Probed, probe};
pub use registry::{KindVisitor, visit};
pub use unavailable::Unavailable;
pub use when::modified_text;
pub use worker::{PeekFailure, PeekWork, PeekWorker, WorkerConfig};
