//! The root machine: every region of the viewer composed, statechart style. It routes each input
//! to its region, lifts the regions' outputs, and owns the few things that cross regions: a
//! file starting to load, a palette command, the chrome's pins.

mod command;
mod model;
mod picture;
mod pins;
mod region;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Choosing, PanelSay, Trashing, Viewer, ViewerIn, ViewerOut, ViewerParams};
