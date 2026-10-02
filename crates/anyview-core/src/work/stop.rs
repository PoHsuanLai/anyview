//! Telling a worker to give up: a flag the caller raises and an optional deadline.

use ds_core::word::Word;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Whether work should go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StopState {
    /// Nothing has asked the work to end.
    Running,
    /// The work is no longer wanted: end it as soon as it is safe to.
    Stopped,
}

/// A request to end work early, handed to `Backend::run`. Clones share one flag, so the caller
/// keeps a clone and raises it while a worker polls its own. The deadline is plain data and this
/// crate never reads a clock: the question "is it past?" takes the instant (`stopped_at`), and
/// the worker passes `Instant::now()`. A back end with its own deadline (a per-render limit)
/// reads `deadline` and uses that instead.
#[derive(Debug, Clone, Default)]
pub struct Stop {
    flag: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl Stop {
    /// A stop that ends only when `request` is called.
    pub fn new() -> Self {
        Self::default()
    }

    /// A stop that also ends at `deadline`.
    pub fn with_deadline(deadline: Instant) -> Self {
        Self {
            flag: Arc::default(),
            deadline: Some(deadline),
        }
    }

    /// Asks every clone's work to end.
    pub fn request(&self) {
        self.flag.store(true, Ordering::Release);
    }

    /// Whether `request` was called on this or any clone; ignores the deadline.
    pub fn stopped(&self) -> StopState {
        if self.flag.load(Ordering::Acquire) {
            StopState::Stopped
        } else {
            StopState::Running
        }
    }

    /// Whether the work should end at `now`: asked to, or `now` is at or past the deadline.
    pub fn stopped_at(&self, now: Instant) -> StopState {
        match (self.stopped(), self.deadline) {
            (StopState::Stopped, _) => StopState::Stopped,
            (StopState::Running, Some(deadline)) if now >= deadline => StopState::Stopped,
            (StopState::Running, Some(_) | None) => StopState::Running,
        }
    }

    /// When the work must end, if it has a deadline.
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
}
