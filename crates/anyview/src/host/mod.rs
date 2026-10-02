//! What the program does for a window: the requests the views make of their host
//! ([`anyview_ui::HostRequest`]) become tasks ([`route`]), and the desktop carries the tasks out
//! through the platform's traits ([`Desktop`]). Routing is pure and decides; the desktop only
//! does.

mod desktop;
mod outcome;
mod route;
mod store;
mod trash;

#[cfg(test)]
mod tests;

pub use desktop::{Desktop, Hosting, LinuxDesktop};
pub use outcome::{Declined, Outcome, report, report_declined};
pub use route::{Carry, Shown, Task, WindowTask, route};
pub use store::{Clock, Store};
pub use trash::{SystemTrash, Trash, TrashError};
