//! The threading contract every back end shares, as a local copy of `anyview_core::work`, which
//! another branch adds. It moves there: delete this file, import the same four names from
//! `anyview_core`, and nothing else in the crate changes. Note that the constructors of [`Stop`]
//! (`new`, `with_deadline`, `raise`) are this copy's own; the real type decides how a stop is made.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Names one request. Every result carries the ticket of the job that made it, and a result whose
/// ticket is not the latest is for work the person has moved on from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Ticket(pub u64);

impl Ticket {
    /// The ticket after this one.
    pub fn next(self) -> Ticket {
        Ticket(self.0.saturating_add(1))
    }
}

/// Whether a stop has been raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StopState {
    /// Keep going.
    Running,
    /// Give up.
    Stopped,
}

/// How running work is told to give up: a flag any clone can raise, and an optional time by which
/// the work must be done. Cloning gives another handle to the same flag. Once raised it stays
/// raised, so each job gets its own.
#[derive(Debug, Clone)]
pub struct Stop {
    flag: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl Stop {
    /// A stop that is never raised until [`Stop::raise`], with no deadline.
    pub fn new() -> Stop {
        Stop {
            flag: Arc::new(AtomicBool::new(false)),
            deadline: None,
        }
    }

    /// A stop that also wants the work done by `deadline`.
    pub fn with_deadline(deadline: Instant) -> Stop {
        Stop {
            deadline: Some(deadline),
            ..Stop::new()
        }
    }

    /// Raises the flag for every clone.
    pub fn raise(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    /// Whether the flag is raised. A passed deadline is not reported here: the back end that
    /// understands a deadline reads it with [`Stop::deadline`].
    pub fn stopped(&self) -> StopState {
        if self.flag.load(Ordering::Relaxed) {
            StopState::Stopped
        } else {
            StopState::Running
        }
    }

    /// When the work must be done, if it must.
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
}

impl Default for Stop {
    fn default() -> Self {
        Stop::new()
    }
}

/// The one contract every back end implements: a document shared between workers, scratch each
/// worker keeps, jobs in and ticketed results out. The binary owns every thread and calls `run`
/// from its pool; a back end never spawns.
pub trait Backend: 'static {
    /// The document, shared by every worker (put in an `Arc`).
    type Doc: Send + Sync;
    /// What one worker keeps between jobs (for PDF, pdfrum's render caches).
    type Worker: Send;
    /// What is asked for.
    type Job: Send;
    /// What comes back, carrying the ticket of its job.
    type Done: Send;

    /// Runs `job` to the end or until `stop` says to give up.
    fn run(doc: &Self::Doc, worker: &mut Self::Worker, job: Self::Job, stop: &Stop) -> Self::Done;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tickets_count_up_and_saturate() {
        assert_eq!(Ticket::default().next(), Ticket(1));
        assert_eq!(Ticket(u64::MAX).next(), Ticket(u64::MAX));
    }

    #[test]
    fn a_raised_stop_stays_raised_in_every_clone() {
        let stop = Stop::new();
        let other = stop.clone();
        assert_eq!(stop.stopped(), StopState::Running);
        other.raise();
        assert_eq!(stop.stopped(), StopState::Stopped);
    }

    #[test]
    fn a_deadline_is_reported_to_the_back_end_not_folded_into_the_flag() {
        let at = Instant::now() + Duration::from_secs(60);
        let stop = Stop::with_deadline(at);
        assert_eq!(stop.deadline(), Some(at));
        assert_eq!(stop.stopped(), StopState::Running);
        assert_eq!(Stop::new().deadline(), None);
    }
}
