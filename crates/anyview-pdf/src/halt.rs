//! Where a `Stop` meets pdfrum: the `Stop`'s own flag is the one pdfrum polls, so a raise ends a
//! draw between the drawn objects of a tile, and a deadline rides on it as pdfrum's budget.

use anyview_core::work::{Stop, StopState};
use pdfrum::{Deadline, LimitExceeded};
use std::time::Instant;

/// A job's stop, ready to hand to pdfrum.
#[derive(Debug)]
pub(crate) struct Halt<'a> {
    stop: &'a Stop,
    deadline: Deadline,
}

impl<'a> Halt<'a> {
    /// pdfrum never lowers a flag, so each job's `Stop` makes its own `Halt` and a worker never
    /// carries one from a job to the next.
    pub(crate) fn new(stop: &'a Stop) -> Halt<'a> {
        let flagged = Deadline::from_flag(stop.flag());
        let deadline = match stop.deadline() {
            Some(at) => flagged.with_budget(at.saturating_duration_since(Instant::now())),
            None => flagged,
        };
        Halt { stop, deadline }
    }

    /// Whether the work should end: the flag is raised or the deadline has passed.
    pub(crate) fn is_up(&self) -> bool {
        self.stop.stopped() == StopState::Stopped || self.deadline.passed()
    }

    /// The deadline for `RenderSession::set_deadline`.
    pub(crate) fn deadline(&self) -> &Deadline {
        &self.deadline
    }

    /// Whether `error` is pdfrum giving up because of this stop (its own flag or budget) rather
    /// than the draw failing; a stop raised just as an unrelated error came back counts too.
    pub(crate) fn ended(&self, error: &pdfrum::Error) -> bool {
        matches!(
            error,
            pdfrum::Error::Limit(LimitExceeded::Stopped { .. } | LimitExceeded::Time { .. })
        ) || self.is_up()
    }
}
