//! The work a window hands its workers, and what comes back. Each job blocks (disk, decoding,
//! highlighting); none touches the UI thread's state. A job that shows pixels uploads them into
//! the texture it was given itself, so a decoded picture never passes through the UI thread: its
//! result is a small document that names the texture.

mod open;
mod run;
mod vocab;

pub(super) use open::Replaces;
pub use open::{Keeping, OpenPort, Opened, Preloaded, WorkLane};
pub use vocab::{Done, Job};
