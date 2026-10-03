//! What the program does for a window: the requests the views make of their host
//! ([`anyview_ui::HostRequest`]) become tasks ([`route`]), and the desktop carries the tasks out
//! through the platform's traits ([`Desktop`]). Routing is pure and decides; the desktop only
//! does.

mod appearance;
mod desktop;
mod media;
mod outcome;
mod pictures;
mod remembering;
mod resume;
mod resume_handed;
mod route;
mod store;
mod trash;
mod watch;

#[cfg(test)]
mod tests;

pub use appearance::{Appearances, look_of};
pub use desktop::{Desktop, Hosting, LinuxDesktop, Services};
pub use media::Media;
pub use outcome::{Declined, Outcome, report, report_declined};
pub use pictures::CachedPictures;
pub use remembering::{REMEMBER_EVERY, Remembering};
pub use resume::HostedResume;
pub use resume_handed::HandedResume;
pub use route::{Carry, Shown, Task, WindowTask, route};
pub use store::{Clock, Store};
pub use trash::{SystemTrash, Trash, TrashError};
pub use watch::{SETTLE, Told, Watcher, WindowWatch};
