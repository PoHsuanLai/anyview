//! The book stage: a chapter of an EPUB or a page of a comic on screen, one at a time, and the
//! moves between them.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{BookIn, BookOut, BookParams, BookStage};
