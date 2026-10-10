//! What a viewer window is wired to, as one value the binary builds and gives to [`Edge::new`]:
//! the pool its work runs on, where its requests go, and each seam the binary has an
//! implementation for. A seam left alone does nothing (nothing is remembered, no recording
//! plays), so a host or a test names only what it has.

use super::abilities::PlatformAbilities;
use super::helpers::{HelperSource, NoHelpers};
use super::media::{MediaHost, NoPlayer};
use super::seams::{
    FileLocks, Forgetful, ImagePlugins, NoImagePlugins, NoLocks, NoVersions, ResumeSource,
    VersionSource,
};
use super::workers::{HostRequest, Workers};
use anyview_peek::{NoStills, StillSource};
use std::sync::Arc;

/// The workers, the request handler and the seams one viewer window is built over. There is no
/// `Default`, since a window cannot do without a pool; [`Services::new`] takes the two things it
/// needs and the `with_*` builders add the rest.
///
/// ```
/// use anyview_ui::{Edge, PlatformAbilities, Services, Work, Workers};
/// use std::sync::Arc;
///
/// struct Inline;
/// impl Workers for Inline {
///     fn submit(&self, work: Work) {
///         work.run();
///     }
/// }
///
/// let services = Services::new(Arc::new(Inline), |_request| {})
///     .with_platform(PlatformAbilities::NONE);
/// let edge = Edge::new(services);
/// assert_eq!(edge.platform(), PlatformAbilities::NONE);
/// ```
#[derive(Clone)]
#[non_exhaustive]
pub struct Services {
    /// The pool that runs the window's work.
    pub workers: Arc<dyn Workers>,
    /// Where the window's requests of the binary go.
    pub on_request: Arc<dyn Fn(HostRequest) + Send + Sync>,
    /// Where each file was left last time.
    pub resume: Arc<dyn ResumeSource>,
    /// Which files refuse a save in place.
    pub locks: Arc<dyn FileLocks>,
    /// The versions kept of a file.
    pub versions: Arc<dyn VersionSource>,
    /// The host's small pictures, shown while a file opens.
    pub first_frames: Arc<dyn StillSource>,
    /// The players a recording starts.
    pub media: Arc<dyn MediaHost>,
    /// The plugins that decode pictures the viewer cannot.
    pub image_plugins: Arc<dyn ImagePlugins>,
    /// The words of the install sheet for each tool.
    pub helpers: Arc<dyn HelperSource>,
    /// The desktop services there are.
    pub platform: PlatformAbilities,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services")
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl Services {
    /// Services over `workers`, handing the window's requests to `on_request`, with every seam
    /// absent: nothing is remembered, every file offers its edits, no recording opens, no tool is
    /// offered and every desktop service is there.
    #[must_use]
    pub fn new(
        workers: Arc<dyn Workers>,
        on_request: impl Fn(HostRequest) + Send + Sync + 'static,
    ) -> Services {
        Services {
            workers,
            on_request: Arc::new(on_request),
            resume: Arc::new(Forgetful),
            locks: Arc::new(NoLocks),
            versions: Arc::new(NoVersions),
            first_frames: Arc::new(NoStills),
            media: Arc::new(NoPlayer),
            image_plugins: Arc::new(NoImagePlugins),
            helpers: Arc::new(NoHelpers),
            platform: PlatformAbilities::default(),
        }
    }

    /// The same services reading where files were left from `source`: without one nothing is
    /// remembered, and every file opens at its start.
    #[must_use]
    pub fn with_resume_source(self, source: Arc<dyn ResumeSource>) -> Services {
        Services {
            resume: source,
            ..self
        }
    }

    /// The same services asking `locks` which files refuse a save in place: without it every file
    /// offers its edits.
    #[must_use]
    pub fn with_locks(self, locks: Arc<dyn FileLocks>) -> Services {
        Services { locks, ..self }
    }

    /// The same services listing a file's kept versions from `source`: without one a file has
    /// none to go back to.
    #[must_use]
    pub fn with_version_source(self, source: Arc<dyn VersionSource>) -> Services {
        Services {
            versions: source,
            ..self
        }
    }

    /// The same services showing the host's small pictures while a file opens: without them a
    /// file shows once it is open.
    #[must_use]
    pub fn with_first_frames(self, source: Arc<dyn StillSource>) -> Services {
        Services {
            first_frames: source,
            ..self
        }
    }

    /// The same services starting players with `host`: without one a recording does not open.
    #[must_use]
    pub fn with_media(self, host: Arc<dyn MediaHost>) -> Services {
        Services {
            media: host,
            ..self
        }
    }

    /// The same services decoding pictures the viewer cannot (HEIC, raw files in full) through
    /// `plugins`: without them a file only a plugin can show is shown as its facts.
    #[must_use]
    pub fn with_image_plugins(self, plugins: Arc<dyn ImagePlugins>) -> Services {
        Services {
            image_plugins: plugins,
            ..self
        }
    }

    /// The same services wording the install sheet from `helpers`: without them no tool is
    /// offered.
    #[must_use]
    pub fn with_helpers(self, helpers: Arc<dyn HelperSource>) -> Services {
        Services { helpers, ..self }
    }

    /// The same services offering only what the platform can do: an action whose desktop service
    /// is absent is not listed, bound or drawn. Without this every service is there.
    #[must_use]
    pub fn with_platform(self, platform: PlatformAbilities) -> Services {
        Services { platform, ..self }
    }
}
