//! Exports and printouts of images, PDFs and text documents. Each format plans its own jobs
//! (`anyview_image::plan_export`, `anyview_pdf::plan_export`, `anyview_text::plan_export`); this
//! crate runs them: a job becomes the bytes of one file, and an export writes each file beside the
//! original under a free name, through a temporary file renamed into place, so a destination is
//! never partial and an existing file is never replaced. A printout is the PDF a printer takes.
//!
//! Everything is blocking and runs on the caller's worker: no runtime, no spawning, no clock.
//! Recordings are not written here (a plugin writes them).
//!
//! Every public item is reached from this root, once.

mod choice;
mod error;
mod name;
mod pdf;
mod pictures;
mod print;
mod produce;
mod raster;
mod run;
mod session;
mod write;

pub use choice::DocumentExport;
pub use error::ExportError;
pub use name::free_beside;
pub use run::{export, printout};
