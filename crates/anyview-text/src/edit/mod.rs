//! Editing text in place. A [`Buffer`] is the text as a piece table over the file's own bytes,
//! with undo and redo; a [`Session`] is a buffer with a caret or a selection, an IME preedit and
//! the commands a key, a click or the clipboard means; [`EditText`] is a file read for editing,
//! or the reason it cannot be. All of it is pure: the surface that draws it lives in
//! `anyview-ui`, and the host writes what [`Session::file_bytes`] gives back.

mod buffer;
mod motion;
mod session;
mod text;

#[cfg(test)]
mod tests;

pub use buffer::{Buffer, Revision};
pub use session::{Motion, Preedit, Selection, Session};
pub use text::{EDIT_BYTES, EditRefusal, EditText};
