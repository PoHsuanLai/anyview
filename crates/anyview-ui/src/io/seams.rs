//! What the binary lends the window to read from: where a file was left last time, the plugins'
//! pictures, the permissions of a file and the versions kept of it. All are blocking (a file read,
//! a cache lookup), so a worker calls them, never the UI thread; all are read-only, and what the
//! window wants kept goes out as a `HostRequest`. The small picture the host already has for a file
//! is `anyview_peek::StillSource`, which the launcher lends its pane as well.

use super::helpers::Need;
use super::job::Opened;
use crate::VersionRow;
use anyview_core::{FilePath, FileStamp, PixelArea, Resume, Sniffed, Source};
use anyview_image::Rgba8;
use std::fmt::Debug;

/// Where a person left each file, read from the store the binary keeps.
pub trait ResumeSource: Debug + Send + Sync + 'static {
    /// What was remembered for `path` when its file looked like `stamp`; `Resume::Nothing` when
    /// nothing was, the file changed since, or the record cannot be read. Blocking.
    fn recall(&self, path: &FilePath, stamp: FileStamp) -> Resume;
}

/// Where a person's views and places are written, to the store the host keeps. The read side is
/// [`ResumeSource`]; a host that gives both has a viewer that remembers.
pub trait ResumeKeeper: Debug + Send + Sync + 'static {
    /// `opened` was shown: it goes first among the files recently viewed. Blocking; a failure is the
    /// keeper's to log, since nobody is waiting for the answer.
    fn viewed(&self, opened: &Opened);

    /// The person is at `resume` in `source`'s file; `Resume::Nothing` forgets the place. Blocking,
    /// and a failure is the keeper's to log.
    fn remember(&self, source: &Source, resume: &Resume);
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

/// Whether the contents of a file shown as a card could be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Readable {
    /// They could, or the format has none to read.
    #[default]
    Yes,
    /// They could not.
    No,
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
