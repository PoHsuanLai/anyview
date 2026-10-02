//! The threads of the program. The binary owns every one of them (libraries never spawn): one
//! bounded pool that runs back-end jobs, and actors that each own a stateful object that cannot
//! be shared (the media player). Results come back to the UI thread through a `Mailbox` that
//! wakes it; the machines there drop stale tickets, the runtime only delivers.

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
