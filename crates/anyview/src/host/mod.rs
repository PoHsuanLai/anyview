//! What the program does for a window: the requests the views make of their host
//! ([`anyview_ui::HostRequest`]) become tasks ([`route`]), and the desktop carries the tasks out
//! through the platform's traits ([`Desktop`]). Routing is pure and decides; the desktop only
//! does.

mod appearance;
mod desktop;
mod documents;
mod editing;
mod feedback;
mod helpers;
mod image_plugins;
mod locks;
mod media;
mod outcome;
mod path_watch;
mod pictures;
mod plugin_registry;
mod remembering;
mod resume;
mod resume_handed;
mod route;
mod saving;
mod store;
mod trash;
mod version_rows;
mod watch;

#[cfg(test)]
mod tests;

pub use appearance::{Appearances, look_of};
pub use desktop::{Desktop, Hosting, PlatformDesktop, Services};
pub use feedback::{
    Doing, log, notice_of, notice_of_declined, report, subject_of, tell, tell_declined,
    tell_problem,
};
pub use helpers::{HelperHost, Listening};
pub use image_plugins::ImageHost;
pub use locks::StoreLocks;
pub use media::Media;
pub use outcome::{Declined, Outcome};
pub use path_watch::{PATH_SETTLE, PathWatch};
pub use pictures::CachedPictures;
pub use plugin_registry::PluginRegistry;
pub use remembering::{REMEMBER_EVERY, Remembering};
pub use resume::HostedResume;
pub use resume_handed::HandedResume;
pub use route::{Carry, Shown, Task, WindowTask, route};
pub use saving::prune_versions;
pub use store::{Clock, Store};
pub use trash::{SystemTrash, Trash, TrashError};
pub use version_rows::HostedVersions;
pub use watch::{SETTLE, Told, Watcher, WindowWatch};
