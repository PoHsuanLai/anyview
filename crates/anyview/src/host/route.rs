//! A request a window made of its host as the task that carries it out. Pure: it reads the file
//! the window shows, never the disk, so every request and every action has a row in the tests.

use super::editing;
use super::outcome::Declined;
use anyview_core::{FileAction, FileName, FilePath, Helper, Resume, Source, Trail, actions_for};
use anyview_export::DocumentExport;
use anyview_store::VersionId;
use anyview_ui::{
    EditRequest, ExportDraft, HostRequest, NaturalSize, Presentation, Probed, VersionKey,
};

/// The file a window shows, as the host last heard of it.
///
/// With it, the trail of its saves: the versions they kept, which undo and redo go through. They
/// belong to one file, so a window that shows another starts a new trail.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown {
    pub(super) file: Option<Probed>,
    pub(super) trail: Trail<VersionId>,
    /// Edits and undos asked for while a save was being written, in the order they came: each
    /// is asked again when the save ends.
    pub(super) queued: Vec<HostRequest>,
}

impl Shown {
    /// What the window shows, once it has told the host.
    pub fn file(&self) -> Option<&Probed> {
        self.file.as_ref()
    }

    /// The window shows `probed` now: the same file again keeps its trail, another starts one.
    fn showing(self, probed: Probed) -> Shown {
        let same = self
            .file
            .as_ref()
            .is_some_and(|shown| shown.source.path() == probed.source.path());
        Shown {
            file: Some(probed),
            trail: if same { self.trail } else { Trail::default() },
            queued: if same { self.queued } else { Vec::new() },
        }
    }

    /// The window is now at `resume` in its file, as the last request said.
    pub fn remembering(self, resume: Resume) -> Shown {
        Shown {
            file: self.file.map(|probed| Probed { resume, ..probed }),
            ..self
        }
    }

    /// The window shows the file `path` names now, after the host moved it: the versions kept
    /// were of the file at its old name, so the trail starts again.
    pub fn moved_to(self, path: FilePath) -> Shown {
        Shown {
            file: self.file.map(|mut probed| {
                probed.source = Source::new(path, probed.source.stamp());
                probed
            }),
            trail: Trail::default(),
            queued: self.queued,
        }
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
    /// Open each of these files in a window of its own, and close this one (the welcome window).
    OpenFiles(Vec<FilePath>),
    /// Tell the window when this file changes on disk, instead of the one it watched.
    Watch(FilePath),
    /// Stop telling the window about changes.
    Unwatch,
    /// Open the file shown in a window of its own presentation, and close this one: the window
    /// is made again rather than resized, since a window cannot change its own frame.
    Reopen(Presentation),
    /// Size the window to its first file's content, if nobody has resized it.
    Size(NaturalSize),
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
    /// Ask the person for a file in the desktop's dialog; what they choose opens in the window.
    PickFile,
    /// Open a web or mail address with the program that handles it.
    OpenLink(String),
    /// Send the file by mail.
    Share(FilePath),
    /// Hand the file to the print dialog: a PDF as it is, anything else laid out as a PDF first.
    Print(Probed),
    /// Move the file to the trash.
    Trash(FilePath),
    /// Give the file another name in its folder.
    Rename { file: FilePath, to: FileName },
    /// Copy the file beside itself under a free name.
    Duplicate(FilePath),
    /// Play the file with no window, from where the person left it.
    PlayInBackground(Probed),
    /// Write an export of an image, a PDF or a text document beside it.
    ExportDocument {
        file: Probed,
        choice: DocumentExport,
    },
    /// Write a media export of the file beside it: a cut, the audio, or the frame on screen.
    ExportMedia {
        file: Probed,
        choice: anyview_core::MediaExport,
    },
    /// Save the file in place with this change, after keeping the original.
    Edit { file: Probed, request: EditRequest },
    /// Put a kept version back as the file; what it is now is kept first.
    Restore { file: FilePath, version: VersionId },
    /// Put back the kept version this key names.
    RevertTo { file: FilePath, key: VersionKey },
    /// Write a copy of the file at `to`, which must not exist.
    SaveCopy { file: FilePath, to: FilePath },
    /// Install this tool through the system's package service, which asks for the password.
    Provide(Helper),
}

/// The task for `request`, and what the window shows afterwards.
pub fn route(shown: Shown, request: HostRequest) -> (Shown, Carry) {
    match request {
        HostRequest::Opened(probed) => {
            let carry = Carry::Desktop(Task::RecordView(probed.clone()));
            (shown.showing(probed), carry)
        }
        HostRequest::CloseWindow => (shown, Carry::Window(WindowTask::Close)),
        HostRequest::PickFile => (shown, Carry::Desktop(Task::PickFile)),
        HostRequest::Reveal(file) => (shown, Carry::Desktop(Task::Reveal(file))),
        HostRequest::Export(draft) => export(shown, draft),
        HostRequest::Present(presentation) => present(shown, presentation),
        HostRequest::OpenUri(uri) => (shown, Carry::Desktop(Task::OpenLink(uri))),
        HostRequest::Provide(helper) => (shown, Carry::Desktop(Task::Provide(helper))),
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
        HostRequest::Edit(request) => editing::edit(shown, request),
        HostRequest::Rewind(rewind) => editing::rewind(shown, rewind),
        HostRequest::RevertTo(key) => editing::revert(shown, key),
        HostRequest::SaveCopy(typed) => editing::save_copy(shown, typed),
        HostRequest::OpenFiles(files) => (shown, Carry::Window(WindowTask::OpenFiles(files))),
        HostRequest::Watch(file) => (shown, Carry::Window(WindowTask::Watch(file))),
        HostRequest::Unwatch => (shown, Carry::Window(WindowTask::Unwatch)),
        HostRequest::SizeWindow(size) => (shown, Carry::Window(WindowTask::Size(size))),
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
            if actions_for(probed.sniffed.kind()).contains(&FileAction::Print) {
                Carry::Desktop(Task::Print(probed.clone()))
            } else {
                Carry::Declined(Declined::NotPrintable)
            }
        }),
        // The viewer opens the export sheet for this; the sheet's answer is `HostRequest::Export`.
        FileAction::Export => declined(shown, Declined::NeedsSheet),
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

/// An export the window's sheet confirmed: a recording's is written by a plugin or the player,
/// every other format's by the export crate.
fn export(shown: Shown, draft: ExportDraft) -> (Shown, Carry) {
    let document = |choice| {
        move |probed: &Probed| {
            Carry::Desktop(Task::ExportDocument {
                file: probed.clone(),
                choice,
            })
        }
    };
    match draft {
        ExportDraft::Media(choice) => about_file(shown, |probed| {
            Carry::Desktop(Task::ExportMedia {
                file: probed.clone(),
                choice,
            })
        }),
        ExportDraft::Raster(choice) => about_file(shown, document(DocumentExport::Raster(choice))),
        ExportDraft::Pdf(choice) => about_file(shown, document(DocumentExport::Pdf(choice))),
        ExportDraft::Text(choice) => about_file(shown, document(DocumentExport::Text(choice))),
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
