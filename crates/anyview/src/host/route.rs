//! A request a window made of its host as the task that carries it out. Pure: it reads the file
//! the window shows, never the disk, so every request and every action has a row in the tests.

use super::outcome::Declined;
use anyview_core::{FileAction, FileName, FilePath, FormatKind, Resume, Source};
use anyview_ui::{ExportDraft, HostRequest, Presentation, Probed};

/// The file a window shows, as the host last heard of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown(Option<Probed>);

impl Shown {
    /// What the window shows, once it has told the host.
    pub fn file(&self) -> Option<&Probed> {
        self.0.as_ref()
    }

    /// The window is now at `resume` in its file, as the last request said.
    pub fn remembering(self, resume: Resume) -> Shown {
        Shown(self.0.map(|probed| Probed { resume, ..probed }))
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
    /// Tell the window when this file changes on disk, instead of the one it watched.
    Watch(FilePath),
    /// Stop telling the window about changes.
    Unwatch,
    /// Open the file shown in a window of its own presentation, and close this one: the window
    /// is made again rather than resized, since a window cannot change its own frame.
    Reopen(Presentation),
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
    /// Play the file with no window, from where the person left it.
    PlayInBackground(Probed),
    /// Write a media export of the file beside it: a cut, the audio, or the frame on screen.
    ExportMedia {
        file: Probed,
        choice: anyview_core::MediaExport,
    },
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
        HostRequest::Export(draft) => export(shown, draft),
        HostRequest::Present(presentation) => present(shown, presentation),
        HostRequest::OpenUri(_) => declined(shown, Declined::OpenUri),
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
        HostRequest::Remember(resume) => {
            let (shown, carry) = about_file(shown, |probed| {
                Carry::Desktop(Task::Remember {
                    source: probed.source.clone(),
                    resume: resume.clone(),
                })
            });
            (shown.remembering(resume), carry)
        }
        HostRequest::Watch(file) => (shown, Carry::Window(WindowTask::Watch(file))),
        HostRequest::Unwatch => (shown, Carry::Window(WindowTask::Unwatch)),
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
        // The viewer opens the export sheet for this; the sheet's answer is `HostRequest::Export`.
        FileAction::Export => declined(shown, Declined::Export),
        FileAction::Rename | FileAction::ConvertTo => declined(shown, Declined::NeedsSheet),
        FileAction::CopyFile => declined(shown, Declined::CopyFile),
        FileAction::SaveCopy
        | FileAction::RevertTo
        | FileAction::RotateLeft
        | FileAction::RotateRight
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical => declined(shown, Declined::Edit),
        FileAction::PlayInBackground => about_file(shown, |probed| {
            Carry::Desktop(Task::PlayInBackground(probed.clone()))
        }),
        FileAction::PlayInMiniWindow => {
            (shown, Carry::Window(WindowTask::Reopen(Presentation::Mini)))
        }
    }
}

/// An export the window's sheet confirmed. Only recordings are written here: the other formats'
/// exports wait for the export pipeline.
fn export(shown: Shown, draft: ExportDraft) -> (Shown, Carry) {
    match draft {
        ExportDraft::Media(choice) => about_file(shown, |probed| {
            Carry::Desktop(Task::ExportMedia {
                file: probed.clone(),
                choice,
            })
        }),
        ExportDraft::Raster(_) | ExportDraft::Pdf(_) | ExportDraft::Text(_) => {
            declined(shown, Declined::Export)
        }
    }
}

/// The window is to be on screen another way. A window is made again to do it; a quick look or
/// a background session is not something an open window becomes.
fn present(shown: Shown, presentation: Presentation) -> (Shown, Carry) {
    match presentation {
        Presentation::Mini | Presentation::Window => {
            about_file(shown, |_| Carry::Window(WindowTask::Reopen(presentation)))
        }
        Presentation::Peek | Presentation::Background => declined(shown, Declined::Present),
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
