//! The effects of the viewer's views: the blocking work a worker does (probing a file, opening
//! it, reading a window of its lines) and the contract the binary's pool carries it out through.
//! A library crate never spawns a thread, so [`Job::run`] only blocks; the binary decides which
//! thread, and a [`Reply`] carries the result back to the window as a machine input.

mod backend;
mod disk;
mod error;
mod job;
mod probe;
mod workers;

pub use backend::{Backend, Stop};
pub(crate) use disk::DiskFiles;
pub use error::OpenError;
pub use job::{Done, Job, OpenLink, Probed};
pub(crate) use probe::probe;
pub use workers::{Edge, HostRequest, Reply, Work, Workers};
