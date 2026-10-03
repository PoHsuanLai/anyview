//! The PDF stage: reading pages, finding text, jumping to a page.

mod finding;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Destination, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
