//! The PDF stage: reading pages, finding text, jumping to a page.

mod finding;
mod model;
mod outline;
mod place;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Destination, PageEdits, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub use outline::{OutlineFold, OutlineIn, OutlineOut, OutlineShape};
pub(crate) use place::{LineDir, nudged};
pub(crate) use place::{end, start};
