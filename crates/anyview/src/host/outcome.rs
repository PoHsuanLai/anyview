//! How a task ended, and the program's one report of it.

use anyview_core::FilePath;

/// A request the program has no way to carry out yet, named so the report says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declined {
    /// The window shows no file yet, so a request about "the file" has none to mean.
    NoFileShown,
    /// The file is already open in this window.
    AlreadyOpen,
    /// Choosing another file needs a file chooser the platform edge does not have.
    PickFile,
    /// Exports are written by the export pipeline, which is not wired to the window.
    Export,
    /// The window must ask a question first (a new name, a format); it does so through a sheet,
    /// and the answer comes back as its own request.
    NeedsSheet,
    /// A mini window needs window stacking in the window layer.
    Present,
    /// The clipboard holds text only; the file itself cannot be put on it.
    CopyFile,
    /// Editing a file in place and keeping its original are not wired.
    Edit,
    /// Only a PDF is handed to the print dialog as it is; the rest print through an export.
    PrintNeedsPdf,
    /// Background playback needs the media player.
    Playback,
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
    /// There was nothing for it to do (no app to open the file with, no way to share it).
    Nothing(&'static str),
    /// It failed; the text says what was being done and what refused.
    Failed(String),
}

/// The program's one log line for something that did not go as asked. `Done` says nothing.
pub fn report(outcome: &Outcome) {
    match outcome {
        Outcome::Done | Outcome::Moved(_) => {}
        Outcome::Nothing(why) => eprintln!("anyview: nothing to do: {why}"),
        Outcome::Failed(why) => eprintln!("anyview: {why}"),
    }
}

/// The log line for a request nobody carries out.
pub fn report_declined(why: Declined) {
    eprintln!("anyview: not carried out: {why:?}");
}
