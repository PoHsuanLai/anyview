//! Markup: what a person writes on the pages of a document and the file keeps as standard
//! annotations: a highlight, an underline, a strike-through, a note, a rectangle, an oval, a line
//! or an arrow, a text box. Everything is placed as thousandths of the page as displayed (from the
//! top left), so a mark stays where it was put at any zoom and on a turned page.

mod mark;
mod mark_color;
mod page;
mod save;
#[cfg(test)]
mod tests;

pub use mark::{Mark, MarkRef, MarkShape, MarkTool, NOTE_SIZE};
pub use mark_color::MarkColor;
pub use page::{PagePoint, PageRect};
pub use save::{FieldFill, MarkupSave};
