//! How often an export says where it is: once for each tenth of a second of the recording, and
//! never backwards, so a caller can show it without smoothing.

use super::request::{ExportProgress, ProgressSink};
use anyview_core::{MediaLength, MediaTime};

/// The media time between two reports.
const EVERY: u64 = 100_000;

/// Turns the many positions an export passes into the few it reports.
pub(super) struct Reporter {
    sink: ProgressSink,
    of: Option<MediaLength>,
    last: Option<MediaTime>,
}

impl Reporter {
    /// A reporter into `sink` for an export that will write `of`.
    pub(super) fn new(sink: ProgressSink, of: Option<MediaLength>) -> Reporter {
        Reporter {
            sink,
            of,
            last: None,
        }
    }

    /// Say `done` if a tenth of a second has passed since the last report.
    pub(super) fn at(&mut self, done: MediaTime) {
        match self.last {
            Some(last) if done.0 < last.0.saturating_add(EVERY) => {}
            Some(_) | None => self.report(done),
        }
    }

    /// Say `done` as the end of the work, whatever was said before.
    pub(super) fn finish(&mut self, done: MediaTime) {
        self.report(done);
    }

    fn report(&mut self, done: MediaTime) {
        let done = self.last.map_or(done, |last| done.max(last));
        self.last = Some(done);
        (self.sink)(ExportProgress { done, of: self.of });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn reports_are_a_tenth_of_a_second_apart_and_never_go_back() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let mut reporter = Reporter::new(
            Arc::new(move |progress| sink.lock().unwrap().push(progress.done.0)),
            None,
        );
        for at in [0, 30_000, 99_999, 100_000, 150_000, 120_000, 250_000] {
            reporter.at(MediaTime(at));
        }
        reporter.finish(MediaTime(200_000));
        assert_eq!(*seen.lock().unwrap(), vec![0, 100_000, 250_000, 250_000]);
    }
}
