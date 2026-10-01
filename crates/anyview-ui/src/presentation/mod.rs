//! How the viewer is on screen: a window, a quick look, a mini window, or no window at all.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{ContentClass, Presentation, PresentationIn, PresentationOut, PresentationParams};
