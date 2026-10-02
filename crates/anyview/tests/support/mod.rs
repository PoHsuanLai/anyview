//! The viewer's first window under the harness with the binary's own wiring: the runtime's pool
//! for the work, the desktop over fakes for the requests, and a real file to open.
#![allow(dead_code, clippy::unwrap_used)]

use anyview::host::{Clock, Desktop, Store, Trash, TrashError};
use anyview::runtime::PoolSize;
use anyview::seam::{NoticeWaker, Workforce};
use anyview::window::{Factory, Opening, Seed, seeded_root};
use anyview_core::FilePath;
use anyview_platform::PrintOutcome;
use anyview_platform::testing::{FakeApps, FakePrinter, FakeReveal, FakeShare};
use anyview_store::Viewed;
use ds::prelude::Appearance;
use ds_harness::{Backend, Clock as HarnessClock, Driver, Harness, HarnessConfig, Viewport};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

pub const VIEW: Viewport = Viewport {
    width: 900,
    height: 600,
    scale_percent: 100,
};

/// A trash that does nothing: no test touches the person's.
struct NoTrash;

impl Trash for NoTrash {
    fn trash(&self, _file: &FilePath) -> Result<(), TrashError> {
        Ok(())
    }
}

/// A fixture of another crate of the workspace.
pub fn fixture(crate_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_dir)
        .join("tests/fixtures")
        .join(name)
}

/// Everything a window needs, kept alive for the test.
pub struct Rig {
    pub harness: Harness,
    pub workforce: Arc<Workforce>,
    pub store: PathBuf,
    /// When `open` began: the stand-in for the process's start.
    pub started: Instant,
    /// How long the program's own wiring took (pool, runtime, desktop, the folder listing).
    pub wired: Duration,
    /// How long it took until the window's first frame was up.
    pub windowed: Duration,
    _runtime: Runtime,
}

impl std::fmt::Debug for Rig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rig").finish_non_exhaustive()
    }
}

/// The first window opened on `file`, wired as the binary wires it.
pub fn open(file: &Path, scratch: &Path) -> Rig {
    let started = Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let workforce = Arc::new(
        Workforce::start(
            PoolSize::exactly(NonZeroUsize::new(2).unwrap()),
            NoticeWaker::default(),
        )
        .unwrap(),
    );
    let store = scratch.join("store");
    let now: Clock = Arc::new(|| Viewed(1_700_000_000));
    let desktop = Desktop::new(
        runtime.handle().clone(),
        FakeApps::default(),
        FakeReveal::default(),
        FakeShare::default(),
        FakePrinter::answering(PrintOutcome::Printed),
        NoTrash,
        Store::new(&store, now),
    );
    let factory = Factory {
        workers: workforce.workers(),
        hosting: Arc::new(desktop),
        appearance: Appearance::default(),
    };
    let opening = Opening::around(FilePath::new(file).unwrap());
    let wired = started.elapsed();
    let config = HarnessConfig::new(VIEW)
        .with_clock(HarnessClock::Virtual)
        .with_backend(Backend::Hybrid)
        .with_context(Seed { factory, opening });
    let harness = Harness::new(seeded_root, config);
    let windowed = started.elapsed();
    Rig {
        harness,
        workforce,
        store,
        started,
        wired,
        windowed,
        _runtime: runtime,
    }
}

/// Advance the window until `done` holds, letting the workers run in real time between steps.
/// Panics with the time it waited if `done` never holds.
pub fn until(
    harness: &mut Harness,
    what: &str,
    mut done: impl FnMut(&mut Harness) -> bool,
) -> Duration {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if done(harness) {
            return started.elapsed();
        }
        harness.advance(Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("{what} did not happen within 20 s:\n{}", harness.html());
}
