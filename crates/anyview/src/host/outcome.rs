//! How a task ended, and the program's one report of it.

use anyview_core::FilePath;
use anyview_store::VersionId;

/// A request the program has no way to carry out yet, named so the report says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declined {
    /// The window shows no file yet, so a request about "the file" has none to mean.
    NoFileShown,
    /// The file is already open in this window.
    AlreadyOpen,
    /// The window must ask a question first (a new name, a format); it does so through a sheet,
    /// and the answer comes back as its own request.
    NeedsSheet,
    /// A window does not become a quick look or a background session: only a window or the mini
    /// window.
    Present,
    /// The clipboard holds text only; the file itself cannot be put on it.
    CopyFile,
    /// The file has no such edit: a recording cannot be turned, a picture has no pages.
    Edit,
    /// An earlier save is still being written; the file is not changed twice at once.
    Busy,
    /// The request waits for the save being written, and is asked again when it ends.
    Queued,
    /// Nothing was edited in this window, so there is nothing to take back.
    NothingToUndo,
    /// Nothing was taken back in this window, so there is nothing to do again.
    NothingToRedo,
    /// Nothing of this kind is laid out on paper (a recording, a table, an archive).
    NotPrintable,
    /// The typed name is not a file name.
    NotAFileName,
}

/// What became of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It was carried out.
    Done,
    /// It was carried out and the file is now at this path.
    Moved(FilePath),
    /// It wrote this new file (an export, a copy), beside the original or where the person said.
    Wrote(FilePath),
    /// The person chose these files in the file dialog.
    Picked(Vec<FilePath>),
    /// The name asked for is taken: nothing was renamed.
    Taken,
    /// There was nothing for it to do (no app to open the file with, no way to share it).
    Nothing(&'static str),
    /// The file plays elsewhere now (with no window): this window's part is done, and it closes.
    Handed,
    /// It failed; the text says what was being done and what refused.
    Failed(String),
    /// The file was saved in place, and this version holds what it was.
    Written { file: FilePath, kept: VersionId },
    /// A save in place wrote nothing, and the file is as it was; the text says what refused.
    NotWritten(String),
}
