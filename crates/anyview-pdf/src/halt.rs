//! Where a `Stop` meets pdfrum: the flag is checked between steps, and the deadline becomes
//! pdfrum's own per-render stop, which also ends a draw between the objects of a page.

use crate::work_shim::{Stop, StopState};
use pdfrum::Deadline;

/// A job's stop, ready to hand to pdfrum.
#[derive(Debug)]
pub(crate) struct Halt<'a> {
    stop: &'a Stop,
    deadline: Deadline,
}

impl<'a> Halt<'a> {
    pub(crate) fn new(stop: &'a Stop) -> Halt<'a> {
        let deadline = match stop.deadline() {
            Some(at) => Deadline::at(at),
            None => Deadline::manual(),
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
}
