//! What the person is told about a task that ended, and the program's one log of it. The wording
//! is decided here from the task and its outcome (pure); `tell` carries it to the window's edge
//! and to stderr, so no other file in the host writes a line of its own for these paths.

use super::outcome::{Declined, Outcome};
use super::route::Task;
use anyview_core::FilePath;
use anyview_ui::{Edge, Notice};

/// What a task was doing, as the person would name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doing {
    /// Moving the file to the trash.
    Trash,
    /// Giving the file another name.
    Rename,
    /// Copying the file beside itself.
    Duplicate,
    /// Writing an export.
    Export,
    /// Writing a copy where the person said.
    SaveCopy,
    /// Printing.
    Print,
    /// Opening the file with another app.
    OpenWith,
    /// Sending the file by mail.
    Share,
    /// Showing the file in the file manager.
    Reveal,
    /// Playing a recording with no window.
    Play,
    /// Asking for a file in the desktop's dialog.
    Pick,
    /// Opening a web address.
    OpenLink,
    /// Saving a change in place, or putting a kept version back.
    Save,
}

impl Doing {
    /// What `task` does, when the person would want to hear how it went.
    pub fn of(task: &Task) -> Option<Doing> {
        match task {
            Task::Trash(_) => Some(Doing::Trash),
            Task::Rename { .. } => Some(Doing::Rename),
            Task::Duplicate(_) => Some(Doing::Duplicate),
            Task::ExportDocument { .. } | Task::ExportMedia { .. } => Some(Doing::Export),
            Task::SaveCopy { .. } => Some(Doing::SaveCopy),
            Task::Print(_) => Some(Doing::Print),
            Task::OpenWith(_) => Some(Doing::OpenWith),
            Task::Share(_) => Some(Doing::Share),
            Task::Reveal(_) => Some(Doing::Reveal),
            Task::PlayInBackground(_) => Some(Doing::Play),
            Task::PickFile => Some(Doing::Pick),
            Task::OpenLink(_) => Some(Doing::OpenLink),
            Task::Edit { .. } | Task::Restore { .. } | Task::RevertTo { .. } => Some(Doing::Save),
            Task::RecordView(_) | Task::Remember { .. } => None,
        }
    }
}

/// The file's name between quotes, for a sentence.
fn quoted(file: &FilePath) -> String {
    file.file_name().map_or_else(
        || "the file".to_owned(),
        |name| format!("\u{201c}{}\u{201d}", name.as_str()),
    )
}

/// The file `task` is about.
pub fn subject_of(task: &Task) -> Option<FilePath> {
    match task {
        Task::Trash(file)
        | Task::Duplicate(file)
        | Task::Reveal(file)
        | Task::Share(file)
        | Task::Rename { file, .. }
        | Task::SaveCopy { file, .. }
        | Task::Restore { file, .. }
        | Task::RevertTo { file, .. } => Some(file.clone()),
        Task::Print(probed)
        | Task::OpenWith(probed)
        | Task::PlayInBackground(probed)
        | Task::ExportDocument { file: probed, .. }
        | Task::ExportMedia { file: probed, .. }
        | Task::Edit { file: probed, .. } => Some(probed.source.path().clone()),
        Task::RecordView(_) | Task::Remember { .. } | Task::PickFile | Task::OpenLink(_) => None,
    }
}

/// What the person is told when `doing` to `subject` ended as `outcome`, or `None` for a quiet end.
pub fn notice_of(doing: Doing, subject: Option<&FilePath>, outcome: &Outcome) -> Option<Notice> {
    let name = subject.map_or_else(|| "the file".to_owned(), quoted);
    match (doing, outcome) {
        (Doing::Trash, Outcome::Done) => Some(Notice::say(format!("Moved {name} to the Trash"))),
        (Doing::Trash, Outcome::Failed(_)) => Some(Notice::say(format!(
            "Couldn\u{2019}t move {name} to the Trash"
        ))),
        (Doing::Rename, Outcome::Moved(to)) => {
            Some(Notice::say(format!("Renamed to {}", quoted(to))))
        }
        (Doing::Rename, Outcome::Taken) => Some(Notice::say(
            "A file with that name already exists. Choose another name.",
        )),
        (Doing::Rename, Outcome::Failed(_)) => {
            Some(Notice::say(format!("Couldn\u{2019}t rename {name}")))
        }
        (Doing::Duplicate, Outcome::Wrote(copy)) => {
            Some(Notice::say(format!("Duplicated as {}", quoted(copy))).revealing(copy.clone()))
        }
        (Doing::Duplicate, Outcome::Failed(_)) => {
            Some(Notice::say(format!("Couldn\u{2019}t duplicate {name}")))
        }
        (Doing::Export, Outcome::Wrote(made)) => {
            Some(Notice::say(format!("Exported as {}", quoted(made))).revealing(made.clone()))
        }
        (Doing::Export, Outcome::Failed(_)) => {
            Some(Notice::say(format!("Couldn\u{2019}t export {name}")))
        }
        (Doing::SaveCopy, Outcome::Wrote(made)) => {
            Some(Notice::say(format!("Saved a copy as {}", quoted(made))).revealing(made.clone()))
        }
        (Doing::SaveCopy, Outcome::Failed(_)) => Some(Notice::say(format!(
            "Couldn\u{2019}t save a copy of {name}"
        ))),
        (Doing::Print, Outcome::Failed(_)) => {
            Some(Notice::say(format!("Couldn\u{2019}t print {name}")))
        }
        (Doing::Print, Outcome::Nothing(_)) => {
            Some(Notice::say("There is no print dialog on this desktop"))
        }
        (Doing::OpenWith, Outcome::Failed(_)) => Some(Notice::say(format!(
            "Couldn\u{2019}t open {name} with another app"
        ))),
        (Doing::OpenWith, Outcome::Nothing(_)) => {
            Some(Notice::say("No other app can open this kind of file"))
        }
        (Doing::Share, Outcome::Failed(_)) => {
            Some(Notice::say(format!("Couldn\u{2019}t share {name}")))
        }
        (Doing::Share, Outcome::Nothing(_)) => {
            Some(Notice::say("Sharing is not available on this desktop"))
        }
        (Doing::Reveal, Outcome::Failed(_)) => {
            Some(Notice::say("Couldn\u{2019}t show the file in its folder"))
        }
        (Doing::Pick, Outcome::Failed(_)) => {
            Some(Notice::say("Couldn\u{2019}t show the file chooser"))
        }
        (Doing::Pick, Outcome::Nothing(_)) => {
            Some(Notice::say("There is no file chooser on this desktop"))
        }
        (Doing::OpenLink, Outcome::Failed(_)) => Some(Notice::say("Couldn\u{2019}t open the link")),
        (Doing::Play, Outcome::Failed(_)) => {
            Some(Notice::say(format!("Couldn\u{2019}t play {name}")))
        }
        (Doing::Save, Outcome::NotWritten(_)) => Some(Notice::say(format!(
            "Couldn\u{2019}t save the change. {name} is unchanged"
        ))),
        (
            Doing::Trash
            | Doing::Rename
            | Doing::Duplicate
            | Doing::Export
            | Doing::SaveCopy
            | Doing::Print
            | Doing::OpenWith
            | Doing::Share
            | Doing::Reveal
            | Doing::Play
            | Doing::Pick
            | Doing::OpenLink
            | Doing::Save,
            _,
        ) => None,
    }
}

/// The notice for a request nobody carries out, when the person should hear of it.
pub fn notice_of_declined(why: Declined) -> Option<Notice> {
    match why {
        Declined::NotAFileName => Some(Notice::say("That is not a valid file name")),
        Declined::Locked => Some(Notice::say(
            "This file is locked. Make it writable to change it.",
        )),
        Declined::NothingToUndo => Some(Notice::say("Nothing to undo")),
        Declined::NothingToRedo => Some(Notice::say("Nothing to redo")),
        Declined::NoFileShown
        | Declined::AlreadyOpen
        | Declined::NeedsSheet
        | Declined::Present
        | Declined::CopyFile
        | Declined::Edit
        | Declined::Busy
        | Declined::Queued
        | Declined::NotPrintable => None,
    }
}

/// The program's one log line: the detail a notice leaves out, for whoever reads stderr.
pub fn log(line: &str) {
    eprintln!("anyview: {line}");
}

/// The log line of an outcome that did not go as asked; `Done` and its kin say nothing.
fn line_of(outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Done
        | Outcome::Moved(_)
        | Outcome::Wrote(_)
        | Outcome::Picked(_)
        | Outcome::Handed
        | Outcome::Written { .. } => None,
        Outcome::Taken => Some("that name is taken".to_owned()),
        Outcome::Nothing(why) => Some(format!("nothing to do: {why}")),
        Outcome::Failed(why) | Outcome::NotWritten(why) => Some(why.clone()),
    }
}

/// Log `outcome` if it did not go as asked.
pub fn report(outcome: &Outcome) {
    if let Some(line) = line_of(outcome) {
        log(&line);
    }
}

/// A task ended: log it if it went wrong, and tell the window what the person should hear.
pub fn tell(edge: &Edge, doing: Option<Doing>, subject: Option<&FilePath>, outcome: &Outcome) {
    report(outcome);
    if let Some(doing) = doing
        && let Some(notice) = notice_of(doing, subject, outcome)
    {
        edge.notify(notice);
    }
}

/// A request was declined: log it, and tell the window when the person should hear.
pub fn tell_declined(edge: &Edge, why: Declined) {
    if why != Declined::Queued {
        log(&format!("not carried out: {why:?}"));
    }
    if let Some(notice) = notice_of_declined(why) {
        edge.notify(notice);
    }
}

/// Something went wrong that no task owns (the clipboard, a watch): log it and tell the window.
pub fn tell_problem(edge: &Edge, detail: &str, notice: Option<Notice>) {
    log(detail);
    if let Some(notice) = notice {
        edge.notify(notice);
    }
}
