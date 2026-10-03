//! Where the person is in a file, kept without a write for every scroll. A window says where
//! the person is whenever a gesture settles, which is often; the store takes a lock and writes a
//! file. So a place waits here for [`REMEMBER_EVERY`], later ones for the same file replace it,
//! and one write is made when the wait ends, on the blocking pool and never on the window's
//! thread. What is waiting when the program ends is written by [`Remembering::flush`].

use super::store::Store;
use anyview_core::{FilePath, Resume, Source};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::runtime::Handle;

/// The most often a file's place is written. A future `viewer.*` setting (FINDINGS).
pub const REMEMBER_EVERY: Duration = Duration::from_millis(500);

/// The places waiting to be written, one per file.
#[derive(Clone)]
pub struct Remembering {
    store: Arc<Store>,
    runtime: Handle,
    every: Duration,
    waiting: Arc<Mutex<HashMap<FilePath, (Source, Resume)>>>,
}

impl std::fmt::Debug for Remembering {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Remembering").finish_non_exhaustive()
    }
}

impl Remembering {
    /// Places kept in `store`, each written at most once in `every`, the waits run on `runtime`.
    pub fn new(store: Arc<Store>, runtime: Handle, every: Duration) -> Remembering {
        Remembering {
            store,
            runtime,
            every,
            waiting: Arc::default(),
        }
    }

    /// `resume` is where the person is in `source` now. The first call for a file starts the
    /// wait; the ones during it only change what is written at its end.
    pub fn note(&self, source: Source, resume: Resume) {
        let path = source.path().clone();
        let starts = self
            .waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path.clone(), (source, resume))
            .is_none();
        if starts {
            let this = self.clone();
            self.runtime.spawn(async move {
                tokio::time::sleep(this.every).await;
                let _joined = tokio::task::spawn_blocking(move || this.write(&path)).await;
            });
        }
    }

    /// Write what waits, now. Blocking: the program calls it as it ends.
    pub fn flush(&self) {
        let paths: Vec<FilePath> = self
            .waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        for path in paths {
            self.write(&path);
        }
    }

    fn write(&self, path: &FilePath) {
        let waiting = self
            .waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(path);
        if let Some((source, resume)) = waiting
            && let Err(error) = self.store.remember(source.path(), source.stamp(), &resume)
        {
            eprintln!("anyview: cannot keep the place: {error}");
        }
    }
}
