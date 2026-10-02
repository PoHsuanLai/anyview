//! The program's windows: what a window is opened with ([`Opening`]), the wiring every window
//! gets (its [`anyview_ui::Edge`] on the shared workers, its host requests carried out) and the
//! root components that put the viewer in a ds-blitz window, the first and the ones opened later
//! on the same event loop.

mod opening;
mod root;
mod seed;

#[cfg(test)]
mod tests;

pub use opening::{Opening, sequence_around};
pub use root::{Inbox, first_root};
pub use seed::{Factory, Seed};
