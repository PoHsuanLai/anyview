//! The system clipboard behind a seam: Copy Path puts text on it, and the text stays there.
//!
//! ds-blitz's clipboard goes through blitz-shell, which makes an `arboard::Clipboard` for each
//! call and drops it at once. On Wayland the selection belongs to the client that set it, so a
//! dropped owner means a selection that vanishes (or, without the data-control protocol, one that
//! never reached the compositor). [`SystemPasteboard`] keeps one owner for the process.

use anyview_ui::Notice;
use std::sync::Mutex;

/// Why the clipboard did not take the text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PasteError(pub String);

/// Somewhere text can be put for the person to paste.
pub trait Pasteboard {
    /// Put `text` on the clipboard, to stay there after this returns.
    ///
    /// # Errors
    /// [`PasteError`] when the desktop's clipboard refuses.
    fn put_text(&self, text: &str) -> Result<(), PasteError>;
}

/// What copying text told: the line for the log when it failed, and the words for the person.
#[derive(Debug, Clone, PartialEq)]
pub struct Copied {
    /// What went wrong, for the log.
    pub problem: Option<String>,
    /// The quiet note the window shows.
    pub notice: Notice,
}

/// Put `text` on `board`, and say how it went.
pub fn copy_text(board: &dyn Pasteboard, text: &str) -> Copied {
    match board.put_text(text) {
        Ok(()) => Copied {
            problem: None,
            notice: Notice::say("Path copied"),
        },
        Err(error) => Copied {
            problem: Some(format!("cannot copy: {error}")),
            notice: Notice::say("Couldn\u{2019}t copy to the clipboard"),
        },
    }
}

/// The desktop's clipboard, held by one owner for the whole process.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPasteboard;

/// The owner of the selection. Dropping it hands the selection back, so it lives in a static.
static OWNER: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

impl Pasteboard for SystemPasteboard {
    fn put_text(&self, text: &str) -> Result<(), PasteError> {
        let mut owner = OWNER
            .lock()
            .map_err(|_| PasteError("the clipboard owner is poisoned".into()))?;
        if owner.is_none() {
            *owner =
                Some(arboard::Clipboard::new().map_err(|error| PasteError(error.to_string()))?);
        }
        let Some(board) = owner.as_mut() else {
            return Err(PasteError("no clipboard".into()));
        };
        board
            .set_text(text.to_owned())
            .map_err(|error| PasteError(error.to_string()))
    }
}
