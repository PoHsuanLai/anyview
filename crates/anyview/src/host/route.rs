//! A request a window made of its host as the task that carries it out. Pure: it reads the file
//! the window shows, never the disk, so every request and every action has a row in the tests.

use super::outcome::Declined;
use anyview_core::{FileAction, FileName, FilePath, FormatKind, Resume, Source};
use anyview_ui::{HostRequest, Probed};

/// The file a window shows, as the host last heard of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown(Option<Probed>);

impl Shown {
    /// What the window shows, once it has told the host.
    pub fn file(&self) -> Option<&Probed> {
        self.0.as_ref()
    }

    /// The window shows the file `path` names now, after the host moved it.
    pub fn moved_to(self, path: FilePath) -> Shown {
        Shown(self.0.map(|mut probed| {
            probed.source = Source::new(path, probed.source.stamp());
            probed
        }))
    }
}

/// What carries a request out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carry {
    /// The window itself, on its own thread: only it can close itself or reach its clipboard.
    Window(WindowTask),
    /// The desktop, on the program's runtime.
    Desktop(Task),
    /// Nobody, and why.
    Declined(Declined),
}

/// Work only the window's own thread can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowTask {
    /// Close this window.
    Close,
    /// Put this text on the clipboard.
    CopyText(String),
}

/// Work for the desktop. Each names the file it is about: the window may move on while a task
/// runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    /// Add the file to the recently viewed.
    RecordView(Probed),
    /// Keep where the person is in the file.
    Remember { source: Source, resume: Resume },
    /// Open the file in the default other program that handles its type.
    OpenWith(Probed),
    /// Show the file in the file manager.
    Reveal(FilePath),
    /// Send the file by mail.
    Share(FilePath),
    /// Hand the PDF to the print dialog.
    Print(Probed),
    /// Move the file to the trash.
    Trash(FilePath),
    /// Give the file another name in its folder.
    Rename { file: FilePath, to: FileName },
    /// Copy the file beside itself under a free name.
    Duplicate(FilePath),
}

/// The task for `request`, and what the window shows afterwards.
pub fn route(shown: Shown, request: HostRequest) -> (Shown, Carry) {
    match request {
        HostRequest::Opened(probed) => {
            let carry = Carry::Desktop(Task::RecordView(probed.clone()));
            (Shown(Some(probed)), carry)
        }
        HostRequest::CloseWindow => (shown, Carry::Window(WindowTask::Close)),
        HostRequest::PickFile => declined(shown, Declined::PickFile),
        HostRequest::Export(_) => declined(shown, Declined::Export),
        HostRequest::Present(_) => declined(shown, Declined::Present),
        HostRequest::Trash => about_file(shown, |probed| {
            Carry::Desktop(Task::Trash(probed.source.path().clone()))
        }),
        HostRequest::Rename(typed) => match FileName::new(typed.as_str()) {
            Ok(to) => about_file(shown, |probed| {
                Carry::Desktop(Task::Rename {
                    file: probed.source.path().clone(),
                    to,
                })
            }),
            Err(_) => declined(shown, Declined::NotAFileName),
        },
        HostRequest::Remember(resume) => about_file(shown, |probed| {
            Carry::Desktop(Task::Remember {
                source: probed.source.clone(),
                resume,
            })
        }),
        HostRequest::Run(action) => run(shown, action),
    }
}

/// A file action the viewer left to its host.
fn run(shown: Shown, action: FileAction) -> (Shown, Carry) {
    let path = |probed: &Probed| probed.source.path().clone();
    match action {
        FileAction::Open => declined_with_file(shown, Declined::AlreadyOpen),
        FileAction::OpenWith => about_file(shown, |probed| {
            Carry::Desktop(Task::OpenWith(probed.clone()))
        }),
        FileAction::RevealInFolder => {
            about_file(shown, |probed| Carry::Desktop(Task::Reveal(path(probed))))
        }
        FileAction::CopyPath => about_file(shown, |probed| {
            Carry::Window(WindowTask::CopyText(
                probed
                    .source
                    .path()
                    .as_path()
                    .to_string_lossy()
                    .into_owned(),
            ))
        }),
        FileAction::Share => about_file(shown, |probed| Carry::Desktop(Task::Share(path(probed)))),
        FileAction::Duplicate => about_file(shown, |probed| {
            Carry::Desktop(Task::Duplicate(path(probed)))
        }),
        FileAction::MoveToTrash => {
            about_file(shown, |probed| Carry::Desktop(Task::Trash(path(probed))))
        }
        FileAction::Print => about_file(shown, |probed| {
            if probed.sniffed.kind() == FormatKind::Pdf {
                Carry::Desktop(Task::Print(probed.clone()))
            } else {
                Carry::Declined(Declined::PrintNeedsPdf)
            }
        }),
        FileAction::Export => declined(shown, Declined::Export),
        FileAction::Rename | FileAction::ConvertTo => declined(shown, Declined::NeedsSheet),
        FileAction::CopyFile => declined(shown, Declined::CopyFile),
        FileAction::SaveCopy
        | FileAction::RevertTo
        | FileAction::RotateLeft
        | FileAction::RotateRight
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical => declined(shown, Declined::Edit),
        FileAction::PlayInBackground | FileAction::PlayInMiniWindow => {
            declined(shown, Declined::Playback)
        }
    }
}

fn declined(shown: Shown, why: Declined) -> (Shown, Carry) {
    (shown, Carry::Declined(why))
}

fn declined_with_file(shown: Shown, why: Declined) -> (Shown, Carry) {
    about_file(shown, |_| Carry::Declined(why))
}

/// `carry` for the file shown, or a refusal when there is none.
fn about_file(shown: Shown, carry: impl FnOnce(&Probed) -> Carry) -> (Shown, Carry) {
    let carry = match shown.file() {
        Some(probed) => carry(probed),
        None => Carry::Declined(Declined::NoFileShown),
    };
    (shown, carry)
}
