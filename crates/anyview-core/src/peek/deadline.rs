//! A peek's time limit as a value a long loop can ask about. It is cooperative: a loop that
//! checks it stops soon after the limit, and a decoder that never returns is not stopped by it
//! (the host replaces that worker instead).

use super::PeekBudget;
use std::time::Instant;

/// The instant a peek must be done by, or no limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Deadline(Option<Instant>);

impl Deadline {
    /// No limit.
    pub const NEVER: Deadline = Deadline(None);

    /// The deadline of a peek that starts at `start` inside `budget`. A budget whose time cannot be
    /// added to an instant has no limit.
    pub fn of(budget: &PeekBudget, start: Instant) -> Deadline {
        Deadline(start.checked_add(budget.time))
    }

    /// Whether `now` is at or past the deadline; the caller reads the clock, once per check.
    pub fn passed(self, now: Instant) -> bool {
        self.0.is_some_and(|limit| now >= limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ByteLen, PixelArea};
    use std::time::Duration;

    #[test]
    fn the_deadline_passes_at_the_budget_time_and_never_before() {
        let budget = PeekBudget {
            bytes: ByteLen(1),
            pixels: PixelArea(1),
            time: Duration::from_millis(500),
        };
        let start = Instant::now();
        let deadline = Deadline::of(&budget, start);
        assert!(!deadline.passed(start + Duration::from_millis(499)));
        assert!(deadline.passed(start + Duration::from_millis(500)));
        assert!(!Deadline::NEVER.passed(start + Duration::from_secs(1_000_000)));
    }
}
