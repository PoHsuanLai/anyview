//! Where the person is in a file, kept without a write for every scroll. A window says where
//! the person is whenever a gesture settles, which is often; the store takes a lock and writes a
//! file. The policy (when a place is worth a write) is [`anyview_ui::Remembering`]; this carries
//! it out: a write is made on the blocking pool and never on the window's thread, and a place
//! that has to wait is written by a task that sleeps until the policy says. What is waiting when
//! the program ends is written by [`PlaceWriter::flush`].

use super::store::Store;
use anyview_core::{FilePath, Resume, Source};
use anyview_ui::{Noted, Remembering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

pub use anyview_ui::REMEMBER_EVERY;

struct Inner {
    store: Arc<Store>,
    runtime: Handle,
    /// Zero of the clock the policy is told the time on.
    origin: Instant,
    policy: Mutex<Remembering>,
    /// Writes handed to the blocking pool and not yet done, so a flush can wait them out.
    in_flight: Mutex<usize>,
    settled: Condvar,
}

/// The places of the files shown, written as the policy says.
#[derive(Clone)]
pub struct PlaceWriter {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for PlaceWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaceWriter").finish_non_exhaustive()
    }
}

impl PlaceWriter {
    /// Places kept in `store`, each written at most once in `every`, the waits run on `runtime`.
    pub fn new(store: Arc<Store>, runtime: Handle, every: Duration) -> PlaceWriter {
        PlaceWriter {
            inner: Arc::new(Inner {
                store,
                runtime,
                origin: Instant::now(),
                policy: Mutex::new(Remembering::new(every)),
                in_flight: Mutex::new(0),
                settled: Condvar::new(),
            }),
        }
    }

    /// `resume` is where the person is in `source` now.
    pub fn note(&self, source: Source, resume: Resume) {
        let path = source.path().clone();
        let noted = {
            let mut policy = self
                .inner
                .policy
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            policy.note(source, resume, self.inner.origin.elapsed())
        };
        match noted {
            Noted::Unchanged | Noted::Replaced => {}
            Noted::Write(source, resume) => self.write_soon(source, resume),
            Noted::Wait { due } => {
                let this = self.clone();
                self.inner.runtime.spawn(async move {
                    tokio::time::sleep(due.saturating_sub(this.inner.origin.elapsed())).await;
                    this.write_waiting(&path);
                });
            }
        }
    }

    /// Write what waits, now, and wait out the writes already on the pool, so nothing is lost
    /// when the runtime goes. Blocking: the program calls it as it ends.
    pub fn flush(&self) {
        let waiting = self
            .inner
            .policy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take_all(self.inner.origin.elapsed());
        for (source, resume) in waiting {
            self.write(&source, &resume);
        }
        let mut in_flight = self
            .inner
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while *in_flight > 0 {
            in_flight = self
                .inner
                .settled
                .wait(in_flight)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn write_waiting(&self, path: &FilePath) {
        let waiting = self
            .inner
            .policy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take(path, self.inner.origin.elapsed());
        if let Some((source, resume)) = waiting {
            self.write_soon(source, resume);
        }
    }

    fn write_soon(&self, source: Source, resume: Resume) {
        let this = self.clone();
        let done = InFlight::start(Arc::clone(&self.inner));
        // The flush waits on the count, not on the handle; a write the runtime drops unrun as the
        // program ends still settles the count, through the guard.
        drop(self.inner.runtime.spawn_blocking(move || {
            this.write(&source, &resume);
            drop(done);
        }));
    }

    fn write(&self, source: &Source, resume: &Resume) {
        if let Err(error) = self
            .inner
            .store
            .remember(source.path(), source.stamp(), resume)
        {
            eprintln!("anyview: cannot keep the place: {error}");
        }
    }
}

/// One write on the pool, counted from its start until it is done or dropped unrun.
struct InFlight(Arc<Inner>);

impl InFlight {
    fn start(inner: Arc<Inner>) -> InFlight {
        *inner
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        InFlight(inner)
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut in_flight = self
            .0
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *in_flight = in_flight.saturating_sub(1);
        self.0.settled.notify_all();
    }
}
