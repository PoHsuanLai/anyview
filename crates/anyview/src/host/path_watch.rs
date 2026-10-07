//! The folders programs are found in, watched for changes: a package installed in a terminal puts
//! its program in one of them, and the viewer looks for its tools again when that happens. One
//! `notify` instance and one thread; a burst of events (a package manager writes many files) is
//! told once, when the folders have been quiet for `settle`.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

/// How long the folders must be quiet before a burst is told. Long enough that a package
/// transaction is over when the viewer looks.
pub const PATH_SETTLE: Duration = Duration::from_millis(750);

/// The watch of the folders on a search path. Dropping it stops the watch and its thread.
pub struct PathWatch {
    _notifier: RecommendedWatcher,
}

impl std::fmt::Debug for PathWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathWatch").finish_non_exhaustive()
    }
}

impl PathWatch {
    /// Watch each folder of `path` (a `PATH`-style list) that exists, and call `changed` from a
    /// thread of its own after each settled burst of events.
    pub fn start(
        path: &OsStr,
        settle: Duration,
        changed: impl Fn() + Send + 'static,
    ) -> Result<PathWatch, notify::Error> {
        let (events, heard) = channel::<()>();
        let mut notifier = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let Ok(event) = event else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            // The thread ended: the watch is gone and nobody is left to tell.
            let _gone = events.send(());
        })?;
        let folders: Vec<PathBuf> = std::env::split_paths(path)
            .filter(|folder| folder.is_absolute() && folder.is_dir())
            .collect();
        for folder in &folders {
            // A folder that cannot be watched costs only the news of that folder.
            let _unwatched = notifier.watch(folder, RecursiveMode::NonRecursive);
        }
        std::thread::Builder::new()
            .name("anyview-path-watch".to_owned())
            .spawn(move || {
                while heard.recv().is_ok() {
                    // Let the burst settle: every event inside the window extends it.
                    loop {
                        match heard.recv_timeout(settle) {
                            Ok(()) => {}
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    changed();
                }
            })
            .map_err(notify::Error::io)?;
        Ok(PathWatch {
            _notifier: notifier,
        })
    }
}
