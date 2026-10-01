//! The viewer's memory on disk: the recently-viewed history and what a person left of each file.
//!
//! Two programs use it at once: the viewer writes, and the launcher reads, even when the viewer
//! is not running. So there is no database and no lock: every file is written whole to a
//! temporary name in its own directory, synced, and renamed over the old one, which a reader sees
//! either complete or not at all.
//!
//! Every public item is reached from this root, once.

mod history;
mod label;
mod viewed;

pub use history::{History, HistoryCap, HistoryEntry, history_after_view};
pub use label::{ResumeLabel, resume_label};
pub use viewed::Viewed;
