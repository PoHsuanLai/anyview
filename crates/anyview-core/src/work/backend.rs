//! The one trait every back end implements to run on a pool worker.

use super::Stop;

/// A back end's work, as the pool runs it. The back end's crate exposes `run` and its work items
/// and never spawns a thread; the binary's pool calls `run` on a worker.
pub trait Backend: 'static {
    /// The open document, shared by every worker (an `Arc` of it travels with each job).
    type Doc: Send + Sync;
    /// One worker's scratch (a renderer's caches that need `&mut`), made once per worker thread.
    type Worker: Send;
    /// One unit of work.
    type Job: Send;
    /// What a job produces; the caller pairs it with the ticket of the load that asked.
    type Done: Send;

    /// Runs `job` against `doc`, giving up early where the back end can once `stop` says so.
    fn run(doc: &Self::Doc, worker: &mut Self::Worker, job: Self::Job, stop: &Stop) -> Self::Done;
}
