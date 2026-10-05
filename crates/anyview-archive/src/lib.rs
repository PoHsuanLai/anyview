//! Archives for the viewer: listings of zip, tar, 7z and compressed streams, extracting one
//! entry, the archive peek, and what an office document's package says about itself.
//!
//! Everything is blocking and runs on the caller's worker: no runtime, no spawning, no clock. A
//! listing reads inside a byte budget, so a huge archive costs a glance what a small one does.
//!
//! Every public item is reached from this root, once.

mod container;
mod entry;
mod error;
mod extract;
mod limit;
mod list;
mod office;
mod peek;
mod sevenz;
mod stream;
mod tar;
mod zip_archive;

pub use entry::{Entry, EntryCount, EntryKind, EntryLimit, Holds, Listing};
pub use error::ArchiveError;
pub use extract::{ExtractLimits, extract};
pub use list::list;
pub use office::{OfficeCount, OfficeLook, Thumbnail, ThumbnailCodec, office_look};
pub use peek::{ArchivePeek, ArchivePeeked, PEEK_ENTRIES};
pub use zip_archive::zip_entries;
