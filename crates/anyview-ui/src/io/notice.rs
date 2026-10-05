//! What the host tells the person about a task that ended: the words, and the file a "Show in
//! Folder" button reveals when there is one.

use anyview_core::FilePath;

/// A short notice for the window to show: the wording is the host's, the drawing the window's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// What happened, in a sentence a person reads.
    pub text: String,
    /// The file the notice is about, offered with "Show in Folder"; none offers no button.
    pub reveal: Option<FilePath>,
}

impl Notice {
    /// A notice of `text` with no button.
    pub fn say(text: impl Into<String>) -> Notice {
        Notice {
            text: text.into(),
            reveal: None,
        }
    }

    /// The same notice offering to show `file` in the file manager.
    pub fn revealing(self, file: FilePath) -> Notice {
        Notice {
            reveal: Some(file),
            ..self
        }
    }
}
