//! What the window asks of its host and what it knows of a content's size: the vocabulary of
//! requests the binary hears.

use super::super::job::Opened;
use crate::{EditRequest, ExportDraft, LoadFailure, Presentation, Rewind, TypedText, VersionKey};
use anyview_core::{FileAction, FilePath, Helper, MarkupSave, PixelSize, Resume};

/// What a content's natural size is measured in, so the window can turn it into logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalSize {
    /// Device pixels, one to one: a picture. On a 2x output it is half as many logical pixels.
    Pixels(PixelSize),
    /// Logical pixels as they are: a PDF's page at 100%, where a point is a logical pixel.
    Points(PixelSize),
    /// Logical pixels as they are, for a window that holds a player and little else (an audio
    /// file): it may be smaller than the least a picture's window is.
    Compact(PixelSize),
}

/// What a different file sizes the window to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeBasis {
    /// The loaded document says its content is this size.
    Natural(NaturalSize),
    /// The document loaded and knows no size of its own (a recording's picture, a picture whose
    /// plugin is missing): the host may read the size from the file's header, off the UI thread.
    Header,
    /// The file did not load: the default window, as for any file with no natural size.
    Default,
}

/// What the window asks of the binary that hosts it: the things the viewer decides to do but
/// cannot do itself (they touch the platform, the file system or the window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRequest {
    /// The window now shows this file: the host records it as viewed, and it is the file the
    /// requests below that name none refer to.
    Opened(Opened),
    /// The file the window was asked to show could not be opened, and the window now says so.
    /// See [`NotOpened`] for when it is sent.
    NotOpened(NotOpened),
    /// Carry out a file action on the open file (reveal it, copy it, open it with, print it…).
    Run(FileAction),
    /// Choose another file to open.
    PickFile,
    /// Close this window.
    CloseWindow,
    /// Write this export of the open file.
    Export(ExportDraft),
    /// Move the open file to the trash.
    Trash,
    /// Rename the open file.
    Rename(TypedText),
    /// Save the open file in place with this change; the host keeps the original first.
    Edit(EditRequest),
    /// Write the open text file in place with this text, as the person edited it; the host keeps
    /// the original first. Its end is told to the window (`Edge::saved`).
    SaveText(TextSave),
    /// Write the open PDF in place with the form fields filled and the marks made on it, appended
    /// to the file; the host keeps the original first.
    SaveMarkup(MarkupSave),
    /// Take back the last edit of the open file, or do it again.
    Rewind(Rewind),
    /// Put this kept version back as the open file.
    RevertTo(VersionKey),
    /// Write a copy of the open file under this name or at this path.
    SaveCopy(TypedText),
    /// Keep where the person is in the open file, for next time. Sent whenever a gesture settles,
    /// so the host may coalesce them.
    Remember(Resume),
    /// Watch this file for changes on disk and tell the window (`Edge::changed`); this replaces
    /// the window's earlier watch, and watching the file already watched is no change.
    Watch(FilePath),
    /// Stop watching: the window has no file open.
    Unwatch,
    /// Show the window this way.
    Present(Presentation),
    /// Open these files, each in a window of its own, and close this window: the welcome window's
    /// answer to a choice or a drop.
    OpenFiles(Vec<FilePath>),
    /// Show this file in the file manager (a notice's "Show in Folder").
    Reveal(FilePath),
    /// Open a web or mail address a link of the open file names, with the program that handles it.
    OpenUri(String),
    /// Install this tool through the system's package service; the answer is `Edge::helped`.
    Provide(Helper),
    /// A different file has loaded (or failed to), and this is what to size the window to. Sent
    /// once for each file the window opens, never for the same file loaded again: the host sizes
    /// the window to it, as it did when the first file opened.
    SizeWindow(SizeBasis),
}

/// A file the viewer was asked to show and could not open: the host may offer another way to
/// open it (another program's, say), since the viewer has none.
///
/// Sent once for each open that fails, when the viewer's own failure card goes up: whether the
/// file could not be read, was damaged, locked, empty, over what the viewer opens or of a kind it
/// has no way to show. A file that opens as facts only (an archive, a font, a recording without a
/// player) did open, and is announced by [`HostRequest::Opened`] instead. A window shown again
/// for the same file (it changed on disk and the new copy fails) sends it again for that load,
/// never for a redraw. The next [`HostRequest::Opened`] supersedes it: a later load of the same
/// file that succeeds announces the file as opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotOpened {
    /// The file that did not open.
    pub file: FilePath,
    /// Why, as the failure card says it.
    pub reason: LoadFailure,
}

/// The text of the open file as the person edited it, whole, as the file is to hold it: its
/// line endings and byte-order mark are the ones it had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSave {
    bytes: Vec<u8>,
}

impl TextSave {
    /// The file's new bytes.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> TextSave {
        TextSave { bytes }
    }

    /// The bytes to write.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
