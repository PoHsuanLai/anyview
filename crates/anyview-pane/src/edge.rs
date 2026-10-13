//! What a pane is wired to: the host's workers and the seams it has an implementation for.

use crate::store::PaneStore;
#[cfg(feature = "player")]
use anyview_media_host::PlayerHost;
use anyview_store::{Clock, Viewed};
use anyview_ui::{PlatformAbilities, Seams, Workers};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// The workers and the [`Seams`] one pane runs over, without the request handler, which the pane
/// owns (what it asks of its host is [`ViewerPane`](crate::ViewerPane)'s `on_request`). A host
/// names the seams it has when it makes the edge ([`PaneEdge::new`]). Every pane of a window shares the host's one pool; a clone of an edge is the
/// same edge, and two edges are equal when they are the same value.
///
/// A seam left alone does nothing: nothing is remembered, no recording plays, no tool is offered.
/// The platform of [`PaneEdge::portable`] has no abilities, so what needs the desktop is not listed.
///
/// ```
/// use anyview_pane::{PaneEdge, PlatformAbilities, Seams, Work, Workers};
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
///
/// let seams = Seams::default().with_platform(PlatformAbilities::NONE);
/// let own = PaneEdge::new(Arc::new(Inline), seams);
/// assert_ne!(own, edge);
/// ```
#[derive(Clone)]
pub struct PaneEdge {
    workers: Arc<dyn Workers>,
    seams: Arc<Seams>,
    /// Where the viewer's store is, once [`PaneEdge::with_store`] named it.
    store: Option<PathBuf>,
    /// The time a recorded view is stamped with.
    clock: Clock,
}

impl PartialEq for PaneEdge {
    fn eq(&self, other: &PaneEdge) -> bool {
        Arc::ptr_eq(&self.seams, &other.seams)
    }
}

impl Eq for PaneEdge {}

impl std::fmt::Debug for PaneEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneEdge")
            .field("seams", &self.seams)
            .finish_non_exhaustive()
    }
}

impl PaneEdge {
    /// An edge over `workers` and the host's `seams`. A seam the host leaves alone does nothing,
    /// and the default platform offers every desktop service: a host with no desktop under it
    /// gives `Seams::default().with_platform(PlatformAbilities::NONE)`, or uses
    /// [`portable`](PaneEdge::portable).
    #[must_use]
    pub fn new(workers: Arc<dyn Workers>, seams: Seams) -> PaneEdge {
        PaneEdge {
            workers,
            seams: Arc::new(seams),
            store: None,
            clock: system_clock(),
        }
    }

    /// An edge over `workers` that asks the OS for nothing: every seam absent, and no desktop
    /// service on offer.
    #[must_use]
    pub fn portable(workers: Arc<dyn Workers>) -> PaneEdge {
        PaneEdge::new(
            workers,
            Seams::default().with_platform(PlatformAbilities::NONE),
        )
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

    /// The same edge playing recordings with `player`: the person's own mpv as a child process,
    /// drawing into the pane. Panes that share a window share one [`MediaHub`](crate::MediaHub),
    /// and so one source of sound at a time. Feature `player`.
    #[cfg(feature = "player")]
    #[must_use]
    pub fn with_player(self, player: PlayerHost) -> PaneEdge {
        self.changed(|seams| seams.with_media(Arc::new(player)))
    }

    fn changed(self, change: impl FnOnce(Seams) -> Seams) -> PaneEdge {
        PaneEdge {
            workers: self.workers,
            seams: Arc::new(change(Arc::unwrap_or_clone(self.seams))),
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
        self.changed(|seams| {
            seams
                .with_resume_source(store.clone())
                .with_resume_keeper(store)
        })
    }

    /// The workers this edge was built over.
    pub(crate) fn workers(&self) -> Arc<dyn Workers> {
        Arc::clone(&self.workers)
    }

    /// The seams this edge was built from, for the pane to wire its own request handler over.
    pub(crate) fn seams(&self) -> Seams {
        (*self.seams).clone()
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
