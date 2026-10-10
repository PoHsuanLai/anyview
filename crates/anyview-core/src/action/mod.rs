//! Actions: the one list a file offers, where each may appear and which keys it has.

mod model;
mod reach;
mod shortcut;
mod spec;

#[cfg(test)]
mod tests;

pub use model::FileAction;
pub use reach::{Reach, reach};
pub use shortcut::{file_action_of, own_keys, shortcut};
