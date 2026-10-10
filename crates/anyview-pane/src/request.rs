//! What a pane asks of its host: the viewer's `HostRequest`s narrowed to what a region of
//! another program's window can mean by them.

use anyview_core::{FileAction, FilePath, Helper, Resume};
use anyview_ui::{
    EditRequest, ExportDraft, HostRequest, NaturalSize, Opened, Presentation, Rewind, SizeBasis,
    TextSave, TypedText, VersionKey,
};

/// A change to the file, or an action on it, that the pane leaves to its host: the pane has no
/// sheets and writes nothing, so the host asks what it needs to (a name, a confirmation) and
/// carries the request out, or declines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRequest {
    /// Carry out a file action on the open file (reveal it, rename it, move it to the trash,
    /// print it…).
    Run(FileAction),
    /// Write this export of the open file.
    Export(ExportDraft),
    /// Move the open file to the trash.
    Trash,
    /// Rename the open file.
    Rename(TypedText),
    /// Save the open file in place with this change.
    Edit(EditRequest),
    /// Write the open text file in place with this text.
    SaveText(TextSave),
    /// Take back the last edit of the open file, or do it again.
    Rewind(Rewind),
    /// Put this kept version back as the open file.
    RevertTo(VersionKey),
    /// Write a copy of the open file under this name or at this path.
    SaveCopy(TypedText),
    /// Show this file in the file manager.
    Reveal(FilePath),
    /// Open a web or mail address a link of the open file names.
    OpenUri(String),
    /// Install this tool through the system's package service.
    Provide(Helper),
}

/// What the pane asks of its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneRequest {
    /// The open file's content is naturally this big: the host may use it for the split ratio.
    SizeHint(NaturalSize),
    /// The viewer asked to be closed (its Close). The pane never closes itself.
    ClosePane,
    /// The viewer wants to be shown another way. Only the small window of a recording is ever
    /// asked for; a window, a quick look and a background session are not things a pane becomes.
    Detach(Presentation),
    /// These files are for the host to open in a place of its own: a recording (until the player
    /// can be hosted in a pane) or the files of a chooser the viewer would have opened. None
    /// means the host is to ask which.
    OpenElsewhere(Vec<FilePath>),
    /// The pane now shows this file; it is the file the requests below that name none refer to.
    Opened(Opened),
    /// Keep where the person is in the open file, for next time. Sent whenever a gesture
    /// settles, so the host may coalesce them.
    // TODO(store lane): the pane ships a store-backed handler for this by default (merge on write
    // under a cross-process lock). Until that store exists the host keeps it, or drops it.
    Remember(Resume),
    /// Watch this file for changes on disk and tell the pane (through the edge's `changed`).
    Watch(FilePath),
    /// Stop watching: the pane has no file open.
    Unwatch,
    /// A change to the file, or an action on it, which the host carries out or declines.
    File(FileRequest),
    /// Esc had nothing left to undo: the keyboard goes back to the host.
    Unfocus,
}

impl PaneRequest {
    /// What `request` is to a pane, or `None` when a pane has nothing to say of it (a window
    /// sized to its file, a window made a quick look: not things a region of a window can do).
    /// Total over `HostRequest`: a new request has to be placed here before this compiles.
    #[must_use]
    pub fn from_host(request: HostRequest) -> Option<PaneRequest> {
        match request {
            HostRequest::SizeWindow(basis) => match basis {
                SizeBasis::Natural(size) => Some(PaneRequest::SizeHint(size)),
                SizeBasis::Header | SizeBasis::Default => None,
            },
            HostRequest::CloseWindow => Some(PaneRequest::ClosePane),
            HostRequest::Present(presentation) => match presentation {
                Presentation::Mini => Some(PaneRequest::Detach(presentation)),
                Presentation::Window
                | Presentation::Peek
                | Presentation::Background
                | Presentation::Pane => None,
            },
            HostRequest::PickFile => Some(PaneRequest::OpenElsewhere(Vec::new())),
            HostRequest::OpenFiles(files) => Some(PaneRequest::OpenElsewhere(files)),
            HostRequest::Opened(opened) => Some(PaneRequest::Opened(opened)),
            HostRequest::Remember(resume) => Some(PaneRequest::Remember(resume)),
            HostRequest::Watch(file) => Some(PaneRequest::Watch(file)),
            HostRequest::Unwatch => Some(PaneRequest::Unwatch),
            HostRequest::Run(action) => file(FileRequest::Run(action)),
            HostRequest::Export(draft) => file(FileRequest::Export(draft)),
            HostRequest::Trash => file(FileRequest::Trash),
            HostRequest::Rename(name) => file(FileRequest::Rename(name)),
            HostRequest::Edit(request) => file(FileRequest::Edit(request)),
            HostRequest::SaveText(save) => file(FileRequest::SaveText(save)),
            HostRequest::Rewind(rewind) => file(FileRequest::Rewind(rewind)),
            HostRequest::RevertTo(version) => file(FileRequest::RevertTo(version)),
            HostRequest::SaveCopy(name) => file(FileRequest::SaveCopy(name)),
            HostRequest::Reveal(path) => file(FileRequest::Reveal(path)),
            HostRequest::OpenUri(address) => file(FileRequest::OpenUri(address)),
            HostRequest::Provide(helper) => file(FileRequest::Provide(helper)),
        }
    }
}

fn file(request: FileRequest) -> Option<PaneRequest> {
    Some(PaneRequest::File(request))
}
