//! What every window of the program shares, and what makes one window its own.

use super::opening::Opening;
use crate::host::{Appearances, HostedResume, HostedVersions, Hosting, Watcher};
use anyview_platform::WindowStacking;
use anyview_ui::{FirstFrameSource, MediaHost, Presentation, ResumeSource, VersionSource, Workers};
use std::sync::Arc;

/// The wiring all windows share: the workers their jobs run on and the host their requests go
/// to.
#[derive(Clone)]
pub struct Factory {
    /// The pool the views' work runs on.
    pub workers: Arc<dyn Workers>,
    /// What carries out a window's requests.
    pub hosting: Arc<dyn Hosting>,
    /// Where files were left, read for every window's probes.
    pub resume: Arc<dyn ResumeSource>,
    /// The kept versions of a file, listed for the Revert To sheet.
    pub versions: Arc<dyn VersionSource>,
    /// The small pictures shown while a file opens.
    pub first_frames: Arc<dyn FirstFrameSource>,
    /// The program's one file watcher, when the system has one.
    pub watcher: Option<Arc<Watcher>>,
    /// How the windows look, and how that changes while they are open.
    pub appearances: Appearances,
    /// Starts a player for a window that shows a recording.
    pub media: Arc<dyn MediaHost>,
    /// Keeping the small window above the others, where the desktop lets a program ask.
    pub stacking: Arc<dyn StackingAsk>,
}

/// Asks the desktop to keep a window above the others. Object safe, so the factory holds one
/// without knowing the platform.
pub trait StackingAsk: Send + Sync + 'static {
    /// Ask for `stacking` for the window just opened, and say what came of it.
    fn ask(&self, stacking: anyview_platform::Stacking) -> anyview_platform::StackingOutcome;
}

impl<T: WindowStacking + Send + Sync + 'static> StackingAsk for T {
    fn ask(&self, stacking: anyview_platform::Stacking) -> anyview_platform::StackingOutcome {
        self.request(stacking)
    }
}

impl Factory {
    /// The wiring of the program: the views read where files were left through `hosting`, and
    /// first frames from `first_frames`.
    pub fn new(
        workers: Arc<dyn Workers>,
        hosting: Arc<dyn Hosting>,
        first_frames: Arc<dyn FirstFrameSource>,
        watcher: Option<Arc<Watcher>>,
        appearances: Appearances,
        media: Arc<dyn MediaHost>,
        stacking: Arc<dyn StackingAsk>,
    ) -> Factory {
        Factory {
            workers,
            resume: Arc::new(HostedResume(Arc::clone(&hosting))),
            versions: Arc::new(HostedVersions(Arc::clone(&hosting))),
            hosting,
            first_frames,
            watcher,
            appearances,
            media,
            stacking,
        }
    }
}

impl std::fmt::Debug for Factory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Factory").finish_non_exhaustive()
    }
}

/// One window: the shared wiring and the file it opens. A window root takes it as its props.
#[derive(Debug, Clone)]
pub struct Seed {
    /// The shared wiring.
    pub factory: Factory,
    /// The file and its neighbours.
    pub opening: Opening,
    /// How the window is on screen.
    pub presentation: Presentation,
}

impl PartialEq for Seed {
    fn eq(&self, other: &Seed) -> bool {
        Arc::ptr_eq(&self.factory.hosting, &other.factory.hosting)
            && Arc::ptr_eq(&self.factory.workers, &other.factory.workers)
            && self.opening == other.opening
            && self.presentation == other.presentation
    }
}
