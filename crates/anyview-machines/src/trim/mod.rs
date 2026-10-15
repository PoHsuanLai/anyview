//! The trim of the open recording: the bar's two handles while it is up, the stretch kept once
//! it is applied, and the steps that can be taken back. A pure state like the picture's edits;
//! the file is written only when the host is asked to (⌘S).

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{MIN_KEPT, MediaTrim, TrimIn, TrimWindow};
