//! What every window of the program shares, and what makes one window its own.

use super::fit::parse_screen;
use super::opening::Opening;
use crate::host::{
    Appearances, HelperHost, HostedResume, HostedVersions, Hosting, ImageHost, Watcher,
};
use anyview_peek::StillSource;
use anyview_platform::WindowStacking;
use anyview_ui::{ImagePlugins, MediaHost, Presentation, ResumeSource, VersionSource, Workers};
use ds_blitz::Extent;
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
    pub first_frames: Arc<dyn StillSource>,
    /// The program's one file watcher, when the system has one.
    pub watcher: Option<Arc<Watcher>>,
    /// How the windows look, and how that changes while they are open.
    pub appearances: Appearances,
    /// Starts a player for a window that shows a recording.
    pub media: Arc<dyn MediaHost>,
    /// The plugins that decode the pictures the viewer cannot (HEIC, a raw file in full).
    pub image_plugins: Arc<dyn ImagePlugins>,
    /// The tools the plugins run, and installing one that is missing, when the program ships the
    /// file that names them.
    pub helpers: Option<Arc<HelperHost>>,
    /// Keeping the small window above the others, where the desktop lets a program ask.
    pub stacking: Arc<dyn StackingAsk>,
    /// The screen windows are fitted to in place of the desktop's, in logical pixels (for tests).
    pub(crate) window_screen: Option<Extent>,
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
        first_frames: Arc<dyn StillSource>,
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
            image_plugins: Arc::new(ImageHost::without_plugins()),
            helpers: None,
            stacking,
            window_screen: None,
        }
    }

    /// The same wiring fitting windows to the screen `text` names, as `WIDTHxHEIGHT` logical
    /// pixels, in place of the desktop's: for tests. Text that names no screen is ignored.
    pub fn with_window_screen(self, text: Option<&str>) -> Factory {
        Factory {
            window_screen: text.and_then(parse_screen),
            ..self
        }
    }

    /// The same wiring offering to install the tools `helpers` knows when a plugin lacks one.
    pub fn with_helpers(self, helpers: Arc<HelperHost>) -> Factory {
        Factory {
            helpers: Some(helpers),
            ..self
        }
    }

    /// The same wiring decoding pictures through `plugins`.
    pub fn with_image_plugins(self, plugins: Arc<dyn ImagePlugins>) -> Factory {
        Factory {
            image_plugins: plugins,
            ..self
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
