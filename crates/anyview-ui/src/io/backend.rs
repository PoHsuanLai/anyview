//! The contract every back end implements: one document shared by every worker, one scratch value
//! per worker, and a job that makes a ticketed result. The work module of `anyview-core` owns
//! this trait (`anyview_core::work`); it is declared here until that module lands, and this file
//! becomes a re-export then.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// One back end.
pub trait Backend: 'static {
    /// The document every worker shares.
    type Doc: Send + Sync;
    /// What one worker keeps between jobs (a render session's caches).
    type Worker: Send;
    /// One unit of work.
    type Job: Send;
    /// What a job makes; it carries the load's ticket, so a late one is dropped by the machine.
    type Done: Send;

    /// Do `job` against `doc`, blocking, giving up early when `stop` says so.
    fn run(doc: &Self::Doc, worker: &mut Self::Worker, job: Self::Job, stop: &Stop) -> Self::Done;
}

/// Asks work to end early: a flag the owner raises, and a deadline the work is given.
#[derive(Debug, Clone, Default)]
pub struct Stop {
    flag: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl Stop {
    /// A stop that is never raised and has no deadline.
    pub fn never() -> Stop {
        Stop::default()
    }

    /// The same stop, ending at `deadline`.
    pub fn until(self, deadline: Instant) -> Stop {
        Stop {
            deadline: Some(deadline),
            ..self
        }
    }

    /// Raise the flag: everything holding this stop should end.
    pub fn raise(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    /// Whether the work should end, at `now`.
    pub fn is_raised(&self, now: Instant) -> bool {
        self.flag.load(Ordering::Relaxed) || self.deadline.is_some_and(|deadline| now >= deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_stop_ends_by_its_flag_or_its_deadline() {
        let now = Instant::now();
        let later = now + Duration::from_secs(1);
        assert!(!Stop::never().is_raised(later), "never");
        let flagged = Stop::never();
        flagged.raise();
        assert!(flagged.is_raised(now), "flag");
        let timed = Stop::never().until(later);
        assert!(!timed.is_raised(now), "before the deadline");
        assert!(timed.is_raised(later), "at the deadline");
    }
}
