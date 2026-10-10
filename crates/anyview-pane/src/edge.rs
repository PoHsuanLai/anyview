//! What a pane is wired to: the host's workers and the seams it has an implementation for.

use anyview_peek::StillSource;
use anyview_ui::{
    FileLocks, HelperSource, ImagePlugins, MediaHost, PlatformAbilities, ResumeSource, Services,
    VersionSource, Workers,
};
use std::sync::Arc;

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

    /// The same edge starting players with `host`. Until a player can be hosted in a pane a
    /// recording opens elsewhere whatever this says, so a host has no use for it yet.
    #[must_use]
    pub fn with_media(self, host: Arc<dyn MediaHost>) -> PaneEdge {
        self.changed(|services| services.with_media(host))
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
        }
    }

    fn changed(self, change: impl FnOnce(Services) -> Services) -> PaneEdge {
        PaneEdge::over(change(Arc::unwrap_or_clone(self.services)))
    }

    /// The services this edge was built from, for the pane to wire its own request handler into.
    pub(crate) fn services(&self) -> Services {
        (*self.services).clone()
    }
}
