//! The seams a viewer window (or a pane) is wired with, as one value: each is something the host
//! has an implementation for. A seam left alone does nothing (nothing is remembered, no recording
//! plays), so a host or a test names only what it has.

use super::helpers::{HelperSource, NoHelpers};
use super::media::{MediaHost, NoPlayer};
use super::seams::{
    FileLocks, Forgetful, ImagePlugins, NoImagePlugins, NoLocks, NoVersions, ResumeKeeper,
    ResumeSource, VersionSource,
};
use anyview_machines::PlatformAbilities;
use anyview_peek::{NoStills, StillSource};
use std::sync::Arc;

/// What the host lends a viewer: where files were left, which files refuse a save, the versions
/// kept, the small pictures, the players, the picture plugins, the words of the install sheet and
/// the desktop services there are. `Seams::default()` has every seam absent: nothing is
/// remembered, every file offers its edits, no recording opens, no tool is offered and every
/// desktop service is there. The `with_*` builders name what the host has.
///
/// ```
/// use anyview_ui::{Edge, PlatformAbilities, Seams, Work, Workers};
/// use std::sync::Arc;
///
/// struct Inline;
/// impl Workers for Inline {
///     fn submit(&self, work: Work) {
///         work.run();
///     }
/// }
///
/// let seams = Seams::default().with_platform(PlatformAbilities::NONE);
/// let edge = Edge::new(Arc::new(Inline), |_request| {}, seams);
/// assert_eq!(edge.platform(), PlatformAbilities::NONE);
/// ```
#[derive(Clone)]
#[non_exhaustive]
pub struct Seams {
    /// Where each file was left last time.
    pub(crate) resume: Arc<dyn ResumeSource>,
    /// Where the views and places are written, when the window keeps them itself instead of
    /// asking its host through `HostRequest::Opened` and `HostRequest::Remember`.
    pub(crate) keeper: Option<Arc<dyn ResumeKeeper>>,
    /// Which files refuse a save in place.
    pub(crate) locks: Arc<dyn FileLocks>,
    /// The versions kept of a file.
    pub(crate) versions: Arc<dyn VersionSource>,
    /// The host's small pictures, shown while a file opens.
    pub(crate) first_frames: Arc<dyn StillSource>,
    /// The players a recording starts.
    pub(crate) media: Arc<dyn MediaHost>,
    /// The plugins that decode pictures the viewer cannot.
    pub(crate) image_plugins: Arc<dyn ImagePlugins>,
    /// The words of the install sheet for each tool.
    pub(crate) helpers: Arc<dyn HelperSource>,
    /// The desktop services there are.
    pub(crate) platform: PlatformAbilities,
}

impl std::fmt::Debug for Seams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seams")
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl Default for Seams {
    /// Every seam absent, and every desktop service on offer.
    fn default() -> Seams {
        Seams {
            resume: Arc::new(Forgetful),
            keeper: None,
            locks: Arc::new(NoLocks),
            versions: Arc::new(NoVersions),
            first_frames: Arc::new(NoStills),
            media: Arc::new(NoPlayer),
            image_plugins: Arc::new(NoImagePlugins),
            helpers: Arc::new(NoHelpers),
            platform: PlatformAbilities::default(),
        }
    }
}

impl Seams {
    /// The same seams reading where files were left from `source`: without one nothing is
    /// remembered, and every file opens at its start.
    #[must_use]
    pub fn with_resume_source(self, source: Arc<dyn ResumeSource>) -> Seams {
        Seams {
            resume: source,
            ..self
        }
    }

    /// The same seams writing the views and places to `keeper` from the workers. The window
    /// then keeps them itself (see [`Edge::keep`](super::workers::Edge::keep)); the host still hears `HostRequest::Opened`.
    #[must_use]
    pub fn with_resume_keeper(self, keeper: Arc<dyn ResumeKeeper>) -> Seams {
        Seams {
            keeper: Some(keeper),
            ..self
        }
    }

    /// The same seams asking `locks` which files refuse a save in place: without it every file
    /// offers its edits.
    #[must_use]
    pub fn with_locks(self, locks: Arc<dyn FileLocks>) -> Seams {
        Seams { locks, ..self }
    }

    /// The same seams listing a file's kept versions from `source`: without one a file has
    /// none to go back to.
    #[must_use]
    pub fn with_version_source(self, source: Arc<dyn VersionSource>) -> Seams {
        Seams {
            versions: source,
            ..self
        }
    }

    /// The same seams showing the host's small pictures while a file opens: without them a
    /// file shows once it is open.
    #[must_use]
    pub fn with_first_frames(self, source: Arc<dyn StillSource>) -> Seams {
        Seams {
            first_frames: source,
            ..self
        }
    }

    /// The same seams starting players with `host`: without one a recording does not open.
    #[must_use]
    pub fn with_media(self, host: Arc<dyn MediaHost>) -> Seams {
        Seams {
            media: host,
            ..self
        }
    }

    /// The same seams decoding pictures the viewer cannot (HEIC, raw files in full) through
    /// `plugins`: without them a file only a plugin can show is shown as its facts.
    #[must_use]
    pub fn with_image_plugins(self, plugins: Arc<dyn ImagePlugins>) -> Seams {
        Seams {
            image_plugins: plugins,
            ..self
        }
    }

    /// The same seams wording the install sheet from `helpers`: without them no tool is
    /// offered.
    #[must_use]
    pub fn with_helpers(self, helpers: Arc<dyn HelperSource>) -> Seams {
        Seams { helpers, ..self }
    }

    /// The same seams offering only what the platform can do: an action whose desktop service
    /// is absent is not listed, bound or drawn. Without this every service is there.
    #[must_use]
    pub fn with_platform(self, platform: PlatformAbilities) -> Seams {
        Seams { platform, ..self }
    }
}
