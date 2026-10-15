//! The one question a long loop asks to know whether to end early.

use super::{Stop, StopState};
use crate::peek::Deadline;
use std::time::Instant;

/// Whether the work a loop is doing should end now. A peek answers it with its [`Deadline`], a
/// viewer job with its [`Stop`]; a loop that takes `&dyn Interrupt` serves both and knows neither.
/// It is cooperative: a loop asks at a grain that is small against the time it may overrun by
/// (an entry, a chunk, a batch of lines), and ends with the part it has made, or with its
/// back end's `Interrupted` error when a part would be wrong. A decoder that has no loop of ours
/// to ask from is not reached by it.
///
/// An answer is a clock read when there is a limit to compare with, and cheap when there is none
/// ([`Deadline::NEVER`] never reads the clock): the callers of a loop that has nothing to stop it
/// pass that.
pub trait Interrupt {
    /// Whether the work should end: asked to, or past its limit.
    fn interrupted(&self) -> bool;
}

impl Interrupt for Deadline {
    fn interrupted(&self) -> bool {
        match self.limit() {
            Some(limit) => Instant::now() >= limit,
            None => false,
        }
    }
}

impl Interrupt for Stop {
    fn interrupted(&self) -> bool {
        match self.deadline() {
            Some(_) => self.stopped_at(Instant::now()) == StopState::Stopped,
            None => self.stopped() == StopState::Stopped,
        }
    }
}

impl<T: Interrupt + ?Sized> Interrupt for &T {
    fn interrupted(&self) -> bool {
        (**self).interrupted()
    }
}
