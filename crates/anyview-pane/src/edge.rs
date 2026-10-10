//! What a pane is wired to: the host's workers and the seams it has an implementation for.

use crate::store::PaneStore;
#[cfg(feature = "player")]
use anyview_media_host::PlayerHost;
use anyview_peek::StillSource;
use anyview_store::{Clock, Viewed};
use anyview_ui::{
    FileLocks, HelperSource, ImagePlugins, MediaHost, PlatformAbilities, ResumeSource, Services,
    VersionSource, Workers,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// The workers and the seams one pane runs over: [`anyview_ui::Services`] without the request
/// handler, which the pane owns (what it asks of its host is [`ViewerPane`](crate::ViewerPane)'s
/// `on_request`). Every pane of a window shares the host's one pool; a clone of an edge is the
/// same edge, and two edges are equal when they are the same value.
///
/// A seam left alone does nothing: nothing is remembered, no recording plays, no tool is offered.
/// The platform of [`PaneEdge::portable`] has no abilities, so what needs the desktop is not listed.
///
/// ```
/// use anyview_pane::{PaneEdge, PlatformAbilities, Work, Workers};
/// use std::sync::Arc;
///
/// #[derive(Debug)]
/// struct Inline;
/// impl Workers for Inline {
///     fn submit(&self, work: Work) {
///         work.run();
///     }
/// }
///
/// let edge = PaneEdge::portable(Arc::new(Inline));
/// assert_eq!(edge, edge.clone());
/// ```
#[derive(Clone)]
pub struct PaneEdge {
    services: Arc<Services>,
    /// Where the viewer's store is, once [`PaneEdge::with_store`] named it.
    store: Option<PathBuf>,
    /// The time a recorded view is stamped with.
    clock: Clock,
}

impl PartialEq for PaneEdge {
    fn eq(&self, other: &PaneEdge) -> bool {
        Arc::ptr_eq(&self.services, &other.services)
    }
}

impl Eq for PaneEdge {}

impl std::fmt::Debug for PaneEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneEdge")
            .field("platform", &self.services.platform)
            .finish_non_exhaustive()
    }
}

impl PaneEdge {
    /// An edge over `workers` that asks the OS for nothing: every seam absent, and no desktop
    /// service on offer.
    #[must_use]
    pub fn portable(workers: Arc<dyn Workers>) -> PaneEdge {
        PaneEdge::over(Services::new(workers, |_request| {}).with_platform(PlatformAbilities::NONE))
    }

    /// The same edge keeping the viewer's memory in the store at `root`: it reads where each file
    /// was left, records each file shown in the recently viewed, and writes where the person is
    /// as gestures settle, all on the host's workers. [`PaneRequest::Remember`](crate::PaneRequest::Remember) is then answered
    /// by the pane and never sent to the host. The store is shared with the viewer's own windows
    /// (give `root` the one they use) and with any other program that opens it: each write
    /// merges only its own file's entry under a lock across processes. Unix only.
    ///
    /// The viewer's own windows keep theirs in [`STORE_FOLDER`](crate::STORE_FOLDER) under the
    /// person's data directory (`$XDG_DATA_HOME`, else `~/.local/share`). The host resolves that
    /// directory, as it does its own, and the pane never reads the environment:
    ///
    /// ```ignore
    /// let data = dirs::data_dir().expect("a home");
    /// let edge = PaneEdge::portable(workers).with_store(data.join(anyview_pane::STORE_FOLDER));
    /// ```
    ///
    /// Without a store a pane remembers nothing itself: the host hears every
    /// [`PaneRequest::Remember`](crate::PaneRequest::Remember) and keeps it, or drops it.
    #[must_use]
    pub fn with_store(mut self, root: impl Into<PathBuf>) -> PaneEdge {
        self.store = Some(root.into());
        self.rewired()
    }

    /// The same edge stamping each view a pane records with `clock`, in seconds since the Unix
    /// epoch. The default is the system clock. It matters to an edge with a store, whichever of
    /// [`with_store`](PaneEdge::with_store) and this comes first.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> PaneEdge {
        self.clock = clock;
        self.rewired()
    }

    /// The same edge reading where files were left from `source`.
    #[must_use]
    pub fn with_resume_source(self, source: Arc<dyn ResumeSource>) -> PaneEdge {
        self.changed(|services| services.with_resume_source(source))
    }

    /// The same edge asking `locks` which files refuse a save in place.
    #[must_use]
    pub fn with_locks(self, locks: Arc<dyn FileLocks>) -> PaneEdge {
        self.changed(|services| services.with_locks(locks))
    }

    /// The same edge listing a file's kept versions from `source`.
    #[must_use]
    pub fn with_version_source(self, source: Arc<dyn VersionSource>) -> PaneEdge {
        self.changed(|services| services.with_version_source(source))
    }

    /// The same edge showing the host's small pictures while a file opens.
    #[must_use]
    pub fn with_first_frames(self, source: Arc<dyn StillSource>) -> PaneEdge {
        self.changed(|services| services.with_first_frames(source))
    }

    /// The same edge starting players with `host`, so a recording plays in the pane. With none (the
    /// default, and `MediaHost`s that say [`MediaSupport::Absent`](anyview_ui::MediaSupport)) a
    /// recording is handed to the host as [`PaneRequest::OpenElsewhere`](crate::PaneRequest).
    #[must_use]
    pub fn with_media(self, host: Arc<dyn MediaHost>) -> PaneEdge {
        self.changed(|services| services.with_media(host))
    }

    /// The same edge playing recordings with `player`: the person's own mpv as a child process,
    /// drawing into the pane. Panes that share a window share one [`MediaHub`](crate::MediaHub),
    /// and so one source of sound at a time. Feature `player`.
    #[cfg(feature = "player")]
    #[must_use]
    pub fn with_player(self, player: PlayerHost) -> PaneEdge {
        self.with_media(Arc::new(player))
    }

    /// The same edge decoding the pictures the viewer cannot through `plugins`.
    #[must_use]
    pub fn with_image_plugins(self, plugins: Arc<dyn ImagePlugins>) -> PaneEdge {
        self.changed(|services| services.with_image_plugins(plugins))
    }

    /// The same edge wording the install sheet from `helpers`. A pane has no sheets, so this only
    /// matters to a host that shows the words itself.
    #[must_use]
    pub fn with_helpers(self, helpers: Arc<dyn HelperSource>) -> PaneEdge {
        self.changed(|services| services.with_helpers(helpers))
    }

    /// The same edge offering only what `platform` can do.
    #[must_use]
    pub fn with_platform(self, platform: PlatformAbilities) -> PaneEdge {
        self.changed(|services| services.with_platform(platform))
    }

    fn over(services: Services) -> PaneEdge {
        PaneEdge {
            services: Arc::new(services),
            store: None,
            clock: system_clock(),
        }
    }

    fn changed(self, change: impl FnOnce(Services) -> Services) -> PaneEdge {
        PaneEdge {
            services: Arc::new(change(Arc::unwrap_or_clone(self.services))),
            store: self.store,
            clock: self.clock,
        }
    }

    /// The same edge with its store (when it has one) opened over the clock it has now.
    fn rewired(self) -> PaneEdge {
        let Some(root) = self.store.clone() else {
            return self;
        };
        let store = Arc::new(PaneStore::at(root, self.clock.clone()));
        self.changed(|services| {
            services
                .with_resume_source(store.clone())
                .with_resume_keeper(store)
        })
    }

    /// The services this edge was built from, for the pane to wire its own request handler into.
    pub(crate) fn services(&self) -> Services {
        (*self.services).clone()
    }
}

/// Seconds since the Unix epoch, on the system clock. A clock before it reads as the epoch itself.
fn system_clock() -> Clock {
    Arc::new(|| {
        Viewed(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        )
    })
}
