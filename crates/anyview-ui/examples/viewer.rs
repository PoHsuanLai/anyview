//! Opens one file in the viewer window, with the files beside it as the list the arrow keys walk.
//! It carries a throwaway pool of worker threads: the library never spawns one, and the real
//! binary owns its own.
//!
//! `cargo run -p anyview-ui --example viewer -- <file> [--dark]`

use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};
use anyview_ui::{Edge, HostRequest, Launch, ViewerApp, Work, Workers};
use ds::prelude::{Appearance, Theme};
use ds_blitz::{AppConfig, AppId, Decorations, launch};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

/// How many threads the example's pool runs: enough for a decode beside a window of lines.
const WORKERS: usize = 3;

/// A pool of threads taking work off one queue.
struct Pool {
    queue: Mutex<Sender<Work>>,
}

impl Pool {
    fn start() -> Pool {
        let (queue, work) = channel::<Work>();
        let work: Arc<Mutex<Receiver<Work>>> = Arc::new(Mutex::new(work));
        for _ in 0..WORKERS {
            let work = Arc::clone(&work);
            thread::spawn(move || {
                loop {
                    let next = work.lock().ok().and_then(|queue| queue.recv().ok());
                    match next {
                        Some(work) => work.run(),
                        None => break,
                    }
                }
            });
        }
        Pool {
            queue: Mutex::new(queue),
        }
    }
}

impl Workers for Pool {
    fn submit(&self, work: Work) {
        if let Ok(queue) = self.queue.lock() {
            let _closed = queue.send(work);
        }
    }
}

/// The regular files in `file`'s folder, sorted by name, with `file` first reached: what ← and →
/// walk.
fn sequence_around(file: &Path) -> Option<Sequence> {
    let current = FilePath::new(file).ok()?;
    let folder = file.parent()?;
    let mut siblings: Vec<PathBuf> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    siblings.sort();
    let entries: Vec<FilePath> = siblings
        .iter()
        .filter_map(|path| FilePath::new(path).ok())
        .collect();
    let entries = NonEmpty::from_vec(entries)?;
    Sequence::starting_at(entries, &current, SequenceOrigin::Selection).ok()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dark = args.iter().any(|arg| arg == "--dark");
    let Some(file) = args.iter().find(|arg| !arg.starts_with("--")) else {
        eprintln!("usage: viewer <file> [--dark]");
        std::process::exit(2);
    };
    let Ok(file) = std::fs::canonicalize(file) else {
        eprintln!("viewer: cannot find {file}");
        std::process::exit(1);
    };
    let Ok(path) = FilePath::new(&file) else {
        eprintln!("viewer: {} is not an absolute path", file.display());
        std::process::exit(1);
    };
    let appearance = Appearance {
        theme: if dark { Theme::Dark } else { Theme::Light },
        ..Appearance::default()
    };
    let edge = Edge::new(Arc::new(Pool::start()), |request| {
        if request == HostRequest::CloseWindow {
            std::process::exit(0);
        }
        eprintln!("viewer: the window asks the host to {request:?}");
    });
    let opened = Launch {
        sequence: sequence_around(&file),
        file: path,
        appearance,
    };
    launch(
        ViewerApp,
        AppConfig::new("anyview", 1000, 700)
            .with_app_id(AppId("dev.anyview.Example".to_owned()))
            .with_decorations(Decorations::Client)
            .with_context(edge)
            .with_context(opened),
    );
}
