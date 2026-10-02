//! The viewer window's views: the root that runs the machines, the chrome that comes with the
//! pointer, the palette, the panel and the sheets. They read the machines' states and send them
//! inputs; every effect goes through `crate::io`.

mod app;
mod chrome;
mod keys;
mod palette;
mod panel;
mod session;
mod sheet;

pub use app::{Launch, ViewerApp, stylesheet};

#[cfg(test)]
mod tests;
