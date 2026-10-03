//! Walking the sequence: ← and → through the files around the one that is open.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Navigate, NavigateIn, NavigateOut};
