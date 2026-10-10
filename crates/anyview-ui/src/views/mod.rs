//! The viewer window's views: the root that runs the machines, the chrome that comes with the
//! pointer, the palette, the panel and the sheets. They read the machines' states and send them
//! inputs; every effect goes through `crate::io`.

mod app;
mod arrive;
mod carry;
mod chrome;
mod context;
mod editing;
mod effects;
mod export;
mod export_options;
mod failed;
mod palette;
mod pane;
mod panel;
mod preloads;
mod press;
mod resize;
mod scrub;
mod session;
mod sheet;
mod shelf;
mod unsaved;
mod welcome;
mod window;

pub use app::{Launch, PaneApp, ViewerApp, stylesheet};
pub use pane::{PaneChrome, PaneLink, PaneSeat, use_pane_link};
pub use welcome::WelcomeApp;

#[cfg(test)]
mod tests;
