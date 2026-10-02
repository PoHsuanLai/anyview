//! The files the windows show, watched for changes on disk. The program has one watcher (one
//! `notify` instance, one thread); each window holds a [`WindowWatch`] that says which file it
//! shows. The watcher looks at the folder of that file, keeps the events that name the file, lets
//! a burst settle (an editor writes twice, or writes beside the file and renames it over) and
//! then tells the window once, from its own thread. The window decides whether the file really
//! differs (`Edge::changed` is one stat), so a spurious event costs little.

use anyview_core::FilePath;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

/// How long the folder must be quiet before a burst of events is told to the window. A future
/// `viewer.*` setting (FINDINGS).
pub const SETTLE: Duration = Duration::from_millis(150);

/// What a window wants to hear when its file changed.
pub type Told = Arc<dyn Fn(FilePath) + Send + Sync>;

/// Which window a watch belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WindowKey(u64);

/// What the program knows of what each window watches.
#[derive(Default)]
struct Table {
    next: u64,
    windows: HashMap<WindowKey, Watching>,
    /// How many windows watch each folder, so the folder is watched once and let go when the last
    /// of them moves on.
    folders: HashMap<PathBuf, usize>,
}

struct Watching {
    file: Option<FilePath>,
    told: Told,
}

struct Shared {
    table: Mutex<Table>,
    notifier: Mutex<RecommendedWatcher>,
}

/// The program's one watcher.
pub struct Watcher {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher").finish_non_exhaustive()
    }
}

impl Watcher {
    /// Start watching: one `notify` instance and the thread that lets bursts settle. `settle` is
    /// the quiet time that ends a burst.
    pub fn start(settle: Duration) -> Result<Watcher, notify::Error> {
        let (events, heard) = channel::<PathBuf>();
        let notifier = notify::recommended_watcher(move |event: notify::Result<Event>| {
            // An error is a watch that went away; the next `watch` of the window replaces it.
            let Ok(event) = event else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            for path in event.paths {
                // The thread ended: the watcher is gone and nobody is left to tell.
                let _gone = events.send(path);
            }
        })?;
        let shared = Arc::new(Shared {
            table: Mutex::new(Table::default()),
            notifier: Mutex::new(notifier),
        });
        let weak = Arc::downgrade(&shared);
        std::thread::Builder::new()
            .name("anyview-watch".to_owned())
            .spawn(move || settle_bursts(&heard, &weak, settle))
            .map_err(notify::Error::io)?;
        Ok(Watcher { shared })
    }

    /// A window's end of the watcher: `told` is called, from the watcher's thread, with the file
    /// whenever it changed.
    pub fn window(&self, told: impl Fn(FilePath) + Send + Sync + 'static) -> WindowWatch {
        let mut table = lock(&self.shared.table);
        let key = WindowKey(table.next);
        table.next += 1;
        table.windows.insert(
            key,
            Watching {
                file: None,
                told: Arc::new(told),
            },
        );
        WindowWatch {
            shared: Arc::clone(&self.shared),
            key,
        }
    }
}

/// One window's watch: the file it shows, or none. Dropping it stops watching.
pub struct WindowWatch {
    shared: Arc<Shared>,
    key: WindowKey,
}

impl std::fmt::Debug for WindowWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowWatch")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl WindowWatch {
    /// Watch `file` instead of what this window watched before; the same file again is no change.
    pub fn watch(&self, file: &FilePath) -> Result<(), notify::Error> {
        let mut table = lock(&self.shared.table);
        let Some(held) = table.windows.get_mut(&self.key) else {
            return Ok(());
        };
        if held.file.as_ref() == Some(file) {
            return Ok(());
        }
        let before = held.file.replace(file.clone());
        if let Some(before) = before {
            self.release(&mut table, &before);
        }
        match file.parent() {
            Some(folder) => self.hold(&mut table, folder.as_path()),
            None => Ok(()),
        }
    }

    /// Watch nothing.
    pub fn unwatch(&self) {
        let mut table = lock(&self.shared.table);
        let Some(held) = table.windows.get_mut(&self.key) else {
            return;
        };
        if let Some(before) = held.file.take() {
            self.release(&mut table, &before);
        }
    }

    fn hold(&self, table: &mut Table, folder: &Path) -> Result<(), notify::Error> {
        let count = table.folders.entry(folder.to_path_buf()).or_insert(0);
        *count += 1;
        if *count > 1 {
            return Ok(());
        }
        lock(&self.shared.notifier).watch(folder, RecursiveMode::NonRecursive)
    }

    fn release(&self, table: &mut Table, file: &FilePath) {
        let Some(folder) = file.parent() else { return };
        let folder = folder.as_path();
        let Some(count) = table.folders.get_mut(folder) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            table.folders.remove(folder);
            // The folder may be gone with its files; there is then nothing left to let go of.
            let _gone = lock(&self.shared.notifier).unwatch(folder);
        }
    }
}

impl Drop for WindowWatch {
    fn drop(&mut self) {
        self.unwatch();
        lock(&self.shared.table).windows.remove(&self.key);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The thread: collect the paths of a burst, and when the folder has been quiet for `settle` tell
/// each window whose file is among them. Ends when the watcher is gone.
fn settle_bursts(heard: &Receiver<PathBuf>, shared: &Weak<Shared>, settle: Duration) {
    let mut burst: HashSet<PathBuf> = HashSet::new();
    loop {
        let waited = if burst.is_empty() {
            heard.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            heard.recv_timeout(settle)
        };
        match waited {
            Ok(path) => {
                burst.insert(path);
            }
            Err(RecvTimeoutError::Timeout) => {
                let Some(shared) = shared.upgrade() else {
                    return;
                };
                tell(&shared, &std::mem::take(&mut burst));
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn tell(shared: &Shared, burst: &HashSet<PathBuf>) {
    let table = lock(&shared.table);
    for held in table.windows.values() {
        if let Some(file) = &held.file
            && burst.contains(file.as_path())
        {
            (held.told)(file.clone());
        }
    }
}
