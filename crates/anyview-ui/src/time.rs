//! Durations from settings, added to the stamps machines are given.

use ds_core::time::stamp::Stamp;
use std::time::Duration;

/// `at` plus `span`, saturating, in the whole milliseconds a [`Stamp`] counts.
pub(crate) fn after(at: Stamp, span: Duration) -> Stamp {
    at.after(u64::try_from(span.as_millis()).unwrap_or(u64::MAX))
}
