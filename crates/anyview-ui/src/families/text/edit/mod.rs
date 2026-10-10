//! Editing text in place: an `EditSurface` around the Source line room, with the caret, the
//! selection and the IME's preedit drawn by us from the surface's geometry. The text, the caret,
//! the selection and the history are `anyview_text::Session`'s; this draws it and turns what the
//! surface hears (keys, text, the clipboard, the IME, the pointer) into the session's commands.

mod act;
mod command;
pub(super) mod lines;
mod place;

#[cfg(test)]
mod tests;
