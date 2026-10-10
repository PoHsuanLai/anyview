//! The PDF stage: reading pages, finding text, jumping to a page.

mod finding;
mod model;
mod place;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Destination, PageEdits, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub(crate) use place::{LineDir, nudged};
pub(crate) use place::{end, start};
