//! The sequence the arrow keys walk: a never-empty list of files with a position that is always
//! valid for it.

mod model;
mod non_empty;
mod step;

#[cfg(test)]
mod tests;

pub use model::{ResultsId, Sequence, SequenceOrigin, SequencePosition};
pub use non_empty::NonEmpty;
pub use step::{Neighbours, SequenceMove, moved, neighbours};
