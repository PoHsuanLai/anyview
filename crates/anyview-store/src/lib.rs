//! The viewer's memory on disk: the recently-viewed history and what a person left of each file.
//!
//! Several programs use it at once: the viewer writes, a terminal that embeds the viewer may
//! write too, and the launcher reads, even when the viewer is not running. So there is no
//! database: every file is written whole to a temporary name in its own directory, synced, and
//! renamed over the old one, which a reader sees either complete or not at all. Readers take no
//! lock. Writers take turns through an advisory lock file in the store root, and each merges only
//! its own entry into what is on disk, so the last writer wins per file, never per store.
//!
//! Every public item is reached from this root, once.
//!
//! A launcher reads the history without the viewer running, and the versions kept before a save
//! in place are listed per file:
//!
//! ```no_run
//! use anyview_store::{STORE_FOLDER, Versions, read_history};
//! use std::path::Path;
//!
//! let data = Path::new("/home/me/.local/share");
//! for viewed in read_history(&data.join(STORE_FOLDER)).entries() {
//!     println!("{}", viewed.path.as_path().display());
//! }
//! let state = Path::new("/home/me/.local/state");
//! let versions = Versions::new(state.join(STORE_FOLDER).join("versions"));
//! for kept in versions.list(Path::new("/home/me/notes.txt"))? {
//!     println!("{}", kept.id);
//! }
//! # Ok::<(), anyview_store::StoreError>(())
//! ```

#![warn(missing_docs)]

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
