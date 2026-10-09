//! The viewer's memory on disk: the recently-viewed history and what a person left of each file.
//!
//! Two programs use it at once: the viewer writes, and the launcher reads, even when the viewer
//! is not running. So there is no database and no lock: every file is written whole to a
//! temporary name in its own directory, synced, and renamed over the old one, which a reader sees
//! either complete or not at all.
//!
//! Every public item is reached from this root, once.

mod attrs;
mod details;
mod error;
mod guard;
mod history;
mod io;
mod label;
mod original;
mod place;
mod prune;
mod reader;
mod record;
mod rekey;
mod save;
mod sweep;
mod versions;
mod viewed;
mod writer;

pub use attrs::is_read_only;
pub use details::{file_details, general_facts};

pub use error::{StoreError, StoreOp};
pub use history::{History, HistoryCap, HistoryEntry, history_after_view};
pub use label::{ResumeLabel, resume_label};
pub use place::{
    copy_new, free_beside, is_free, is_taken, link_new, partial_beside, rename_noreplace,
};
pub use reader::{HistoryRead, read_history};
pub use save::{BackedUp, Durability, Pending, Written};
pub use sweep::sweep_leftovers;
pub use versions::{DEFAULT_CAP, DEFAULT_KEEP, KeepPeriod, SavedAt, Version, VersionId, Versions};
pub use viewed::Viewed;
pub use writer::{StoreWriter, ViewRecorded};

/// The folder under a person's data directory that holds the store, which the viewer writes and
/// the launcher reads (`<data>/anyview`).
pub const STORE_FOLDER: &str = "anyview";
