//! The viewer window's views: the root that runs the machines, the chrome that comes with the
//! pointer, the palette, the panel and the sheets. They read the machines' states and send them
//! inputs; every effect goes through `crate::io`.

mod app;
mod arrive;
mod carry;
mod chrome;
mod effects;
mod failed;
mod keys;
mod palette;
mod panel;
mod preloads;
mod scrub;
mod session;
mod sheet;
mod shelf;
mod welcome;
mod window;

pub use app::{Launch, ViewerApp, stylesheet};
pub use welcome::WelcomeApp;

#[cfg(test)]
mod tests;
