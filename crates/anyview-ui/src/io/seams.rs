//! What the binary lends the window to read from: where a file was left last time, and the small
//! picture the host already has for it. Both are blocking (a file read, a cache lookup), so a
//! worker calls them, never the UI thread; both are read-only, and what the window wants kept
//! goes out as a `HostRequest`.

use super::helpers::Need;
use crate::sheet::VersionRow;
use anyview_core::{Facts, FilePath, FileStamp, PixelArea, Resume, Sniffed, Source};
use anyview_image::Rgba8;
use std::fmt::Debug;

/// Where a person left each file, read from the store the binary keeps.
pub trait ResumeSource: Debug + Send + Sync + 'static {
    /// What was remembered for `path` when its file looked like `stamp`; `Resume::Nothing` when
    /// nothing was, the file changed since, or the record cannot be read. Blocking.
    fn recall(&self, path: &FilePath, stamp: FileStamp) -> Resume;
}

/// The cheap first frame of a picture the host has to hand (the thumbnail cache), shown while
/// the file decodes.
pub trait FirstFrameSource: Debug + Send + Sync + 'static {
    /// The small picture of `source` as it is now, upright, or `None` when the host has none
    /// for this version of the file. Blocking.
    fn picture(&self, source: &Source) -> Option<Rgba8>;
}

/// What asking the plugins for a picture of a file came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPicture {
    /// A plugin decoded it: straight RGBA, upright, within the area asked for.
    Pixels(Rgba8),
    /// No installed plugin serves this kind of file, or its tool is missing: the row says what
    /// would, and carries the tool the viewer can offer to install.
    Missing(Need),
    /// No plugin serves it and none is known to: the caller shows what it has.
    Unserved,
    /// A plugin serves it and could not make the picture; this says why.
    Failed(String),
}

/// The plugins that decode the pictures the viewer has no decoder for (HEIC, and a raw file in full),
/// each a separate program the person installed (CONVENTIONS section 15).
pub trait ImagePlugins: Debug + Send + Sync + 'static {
    /// The picture of `source`, a file of `sniffed`'s type, with at most `max_area` pixels. Blocking:
    /// it starts a program and waits for it, so a worker calls it, never the UI thread.
    fn decode(&self, source: &Source, sniffed: &Sniffed, max_area: PixelArea) -> PluginPicture;
}

/// What the host's light tier makes of a file the viewer has no stage for: the rows it lists for
/// the file in the launcher's pane, and what is inside it when it holds things.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileCard {
    /// The rows: a friendly kind, the size, and what the format itself says (a font's family, an
    /// archive's entry count, a folder's tally).
    pub facts: Facts,
    /// The entries of an archive, one line each, in the archive's own order.
    pub listing: Vec<String>,
    /// Whether the file's contents could not be read: it looks damaged.
    pub unreadable: Readable,
}

/// Whether a card's contents could be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Readable {
    /// They could, or the format has none to read.
    #[default]
    Yes,
    /// They could not.
    No,
}

/// The light tier the host links (`anyview-peek`), lent to the views as cards for the files they
/// have no stage for, so a font, an archive or a folder shows what its peek shows.
pub trait FileCards: Debug + Send + Sync + 'static {
    /// The card of `source`, a file of `sniffed`'s type, or `None` when the host has none.
    /// Blocking.
    fn card(&self, source: &Source, sniffed: &Sniffed) -> Option<FileCard>;
}

/// The default `FileCards`: the host has none, so the views list what they know themselves.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoCards;

impl FileCards for NoCards {
    fn card(&self, _source: &Source, _sniffed: &Sniffed) -> Option<FileCard> {
        None
    }
}

/// Whether the window may write a file in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FileAccess {
    /// Edits are saved in place.
    #[default]
    Writable,
    /// The file refuses a save in place (no write bit, another owner, a read-only mount): its
    /// edits are not offered.
    ReadOnly,
}

/// Which files the person may change, read from the permissions the system keeps.
pub trait FileLocks: Debug + Send + Sync + 'static {
    /// How `path` can be written now. A file that cannot be looked at is `Writable`: opening it
    /// fails with its own error. Blocking.
    fn access(&self, path: &FilePath) -> FileAccess;
}

/// The default `FileLocks`: the host reads no permissions, so every file offers its edits.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoLocks;

impl FileLocks for NoLocks {
    fn access(&self, _path: &FilePath) -> FileAccess {
        FileAccess::Writable
    }
}

/// The kept versions of a file, listed from the store the binary keeps.
pub trait VersionSource: Debug + Send + Sync + 'static {
    /// The versions kept of `path`, newest first; none when there are none or the store cannot
    /// be read. Blocking.
    fn list(&self, path: &FilePath) -> Vec<VersionRow>;
}

/// The default `ResumeSource`: nothing is remembered, so every file opens at its start.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Forgetful;

impl ResumeSource for Forgetful {
    fn recall(&self, _path: &FilePath, _stamp: FileStamp) -> Resume {
        Resume::Nothing
    }
}

/// The default `FirstFrameSource`: the host has no pictures, so a file shows once it is open.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoPictures;

impl FirstFrameSource for NoPictures {
    fn picture(&self, _source: &Source) -> Option<Rgba8> {
        None
    }
}

/// The default `ImagePlugins`: none are installed and none is known, so a file only the plugins can
/// show is shown as it is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoImagePlugins;

impl ImagePlugins for NoImagePlugins {
    fn decode(&self, _source: &Source, _sniffed: &Sniffed, _max_area: PixelArea) -> PluginPicture {
        PluginPicture::Unserved
    }
}

/// The default `VersionSource`: nothing is kept, so there is nothing to go back to.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoVersions;

impl VersionSource for NoVersions {
    fn list(&self, _path: &FilePath) -> Vec<VersionRow> {
        Vec::new()
    }
}
