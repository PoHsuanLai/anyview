//! Undo and redo for one file in one window: the versions its saves kept, in the order to go
//! back through them. Generic over what names a kept version, so the pure machine needs nothing
//! of the store that makes them.

mod model;
mod step;

#[cfg(test)]
mod tests;

pub use model::{Trail, TrailIn, TrailOut, TrailStacks};
