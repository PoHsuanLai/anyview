//! The viewer's pure vocabulary: what a file is, how it is told apart, the units the viewer counts
//! in, the sequence arrow keys walk, the actions, edits and exports a file offers, and the view
//! memory a file keeps. No I/O, no clock, no renderer: effects belong to the crates above.

mod error;
pub mod kind;
pub mod source;
pub mod units;

pub use error::CoreError;
