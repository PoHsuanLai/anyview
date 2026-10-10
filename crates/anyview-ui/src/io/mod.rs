//! The effects of the viewer's views: the blocking work a worker does (probing a file, opening
//! it, reading a window of its lines) and the contract the binary's pool carries it out through.
//! A library crate never spawns a thread, so [`Job::run`] only blocks; the binary decides which
//! thread, and a [`Reply`] carries the result back to the window as a machine input.

mod abilities;
mod error;
mod folder;
mod helpers;
mod job;
mod media;
mod notice;
mod probe;
mod seams;
mod services;
mod workers;

pub use abilities::{DesktopService, PlatformAbilities};
pub use anyview_core::work::{Backend, Stop};
pub use error::OpenError;
pub use folder::folder_sequence;
pub use helpers::{HelperSource, HelperWords, Need};
pub use job::{Done, Job, OpenPort, Opened, Preloaded, WorkLane};
pub(crate) use media::MediaPort;
pub use media::{
    MediaHost, MediaLine, MediaNotice, MediaPlayback, MediaStart, MediaStarted, MediaWake,
    SlotPixels,
};
pub use notice::Notice;
pub(crate) use probe::probe;
pub use seams::{
    FileAccess, FileLocks, ImagePlugins, PluginPicture, Readable, ResumeSource, VersionSource,
};
pub use services::Services;
pub use workers::{Edge, HostRequest, NaturalSize, Reply, SizeBasis, Work, WorkKind, Workers};
