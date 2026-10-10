//! The threads of the program: one bounded pool that runs back-end jobs, and actors that each
//! own a stateful object that cannot be shared (the media player). The viewer binary starts the
//! pool; the one library that starts a thread of its own is `anyview-media-host`, for the player,
//! and it does so through [`Actor`] here. No other library spawns.
//!
//! Results come back to the UI thread through a `Mailbox` that wakes it; the machines there drop stale tickets, the runtime only delivers.

mod actor;
mod error;
mod mailbox;
mod pool;
mod runner;

#[cfg(test)]
mod tests;

pub use actor::{Actor, ActorBody, ActorWake, Flow};
pub use error::RuntimeError;
pub use mailbox::{Mailbox, Outbox, UiWaker};
pub use pool::{Lane, Pool, PoolSize};
pub use runner::{JobHandle, JobOutcome, JobPanic, Runner};
