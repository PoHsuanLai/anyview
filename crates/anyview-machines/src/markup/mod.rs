//! What the person writes on the open PDF and has not saved: the form fields they filled, the marks
//! they made (a highlight, a note, a shape), the field the keyboard is in, the mark picked, and
//! the steps that can be taken back. A pure state like the picture's edits; the file is written
//! only when the host is asked to (⌘S).

mod form;
mod marks;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Allowance, Draft, MarkupIn, MarkupOut, PdfMarkup, Using};
