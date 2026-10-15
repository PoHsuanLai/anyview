//! How work leaves the window and its result comes back. The window never runs a [`Job`](super::job::Job) itself
//! and the library never spawns a thread: it hands each [`Work`] to the binary's [`Workers`]
//! (its one bounded pool), a worker calls [`Work::run`], and the [`Done`](super::job::Done) it makes is posted to
//! the window's mailbox through the [`Reply`] the work carries. Results are machine inputs with
//! the load's ticket, so one that arrives after the person left the file is a listed no-op.

mod edge;
mod requests;
mod work;

pub use edge::Edge;
pub use requests::{HostRequest, NaturalSize, NotOpened, SizeBasis, TextSave};
pub use work::{Reply, Work, WorkKind, Workers};

#[cfg(test)]
mod tests;
