//! Where the person is going when changes that are not saved are in the way.

use crate::navigate::NavigateIn;
use anyview_core::FilePath;

/// Where the person is going when the open file has changes that are not saved (a picture's
/// edits, a text's): the question is asked first, and they go on once it is answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Departure {
    /// Closing the window.
    Close,
    /// Opening this file.
    Open(FilePath),
    /// Opening these files, as the file chooser answered.
    Chosen(Vec<FilePath>),
    /// Opening these files, as dropped on the window.
    Dropped(Vec<FilePath>),
    /// Walking to another file of the list.
    Walk(NavigateIn),
    /// Finishing the editing of a text, to read it again.
    Finish,
}
