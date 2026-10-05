//! The effects of the viewer's views: the blocking work a worker does (probing a file, opening
//! it, reading a window of its lines) and the contract the binary's pool carries it out through.
//! A library crate never spawns a thread, so [`Job::run`] only blocks; the binary decides which
//! thread, and a [`Reply`] carries the result back to the window as a machine input.

mod error;
mod folder;
mod job;
mod media;
mod probe;
mod seams;
mod workers;

pub use anyview_core::work::{Backend, Stop};
pub use error::OpenError;
pub use folder::folder_sequence;
pub use job::{Done, Job, OpenLink, Preloaded, Probed, WorkLane};
pub(crate) use media::MediaPort;
pub use media::{
    MediaHost, MediaLine, MediaNotice, MediaPlayback, MediaStart, MediaStarted, MediaWake,
    SlotPixels,
};
pub(crate) use probe::probe;
pub use seams::{FirstFrameSource, ResumeSource};
pub use workers::{Edge, HostRequest, Reply, Work, WorkKind, Workers};
