//! The runtime's one error.

use thiserror::Error;

/// Why the runtime could not do what was asked.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// The operating system refused a new thread.
    #[error("cannot start thread {name}: {source}")]
    Spawn {
        /// The thread's name.
        name: String,
        /// What the system said.
        source: std::io::Error,
    },
    /// The actor's thread has ended (its body quit or panicked), so it takes no more commands.
    #[error("the actor has ended")]
    ActorEnded,
}
