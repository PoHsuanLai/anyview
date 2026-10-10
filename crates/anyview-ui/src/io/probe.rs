//! Probing: what a file is, read from its first 4 KiB (and, for a zip, its entries), and which stage shows it.

use super::error::OpenError;
use super::job::Opened;
use super::seams::FileAccess;
use crate::StageFamily;
use crate::families::family_of;
use anyview_core::{ByteLen, FilePath, FileStamp, FormatKind, ModTime, Resume, Source};
use anyview_fs::OnDisk;
use anyview_peek::PeekError;
use std::io::ErrorKind;

/// Look at the file `path` names: stat it, read its head, sniff it. Blocking. The sniffing is the
/// light tier's (`anyview_peek::probe`), so the viewer and the launcher's pane tell a zip document
/// from an archive by the same rule.
pub(crate) fn probe(path: &FilePath) -> Result<Opened, OpenError> {
    let probed = anyview_peek::probe(path.on_disk()).map_err(open_error)?;
    let stamp = probed.input.stamp();
    let family = family_of(probed.sniffed.kind());
    let folder = probed.sniffed.kind() == FormatKind::Folder;
    if stamp.len == ByteLen(0) && !folder && !holds_text(family) {
        return Err(OpenError::Empty);
    }
    Ok(Opened {
        resume: Resume::Nothing,
        access: FileAccess::Writable,
        family,
        source: Source::new(path.clone(), stamp),
        sniffed: probed.sniffed,
    })
}

/// What a probe that failed means to the load machine: the probe fails only for a path that is not
/// there or cannot be read.
fn open_error(error: PeekError) -> OpenError {
    match error {
        PeekError::Missing { .. } => OpenError::Read(ErrorKind::NotFound),
        PeekError::Unreadable { kind, .. } | PeekError::Folder { kind, .. } => {
            OpenError::Read(kind)
        }
        PeekError::Image(_)
        | PeekError::Text(_)
        | PeekError::Media { .. }
        | PeekError::Archive(_)
        | PeekError::Font(_)
        | PeekError::Pdf(_)
        | PeekError::OverBudget { .. }
        | PeekError::WrongKind { .. } => OpenError::Unrecognised,
    }
}

/// Whether a file of `family` can be a blank document: only text can, and a picture, a document
/// or a recording of no bytes is a file that failed to be written.
fn holds_text(family: StageFamily) -> bool {
    match family {
        StageFamily::Text | StageFamily::Table | StageFamily::Tree => true,
        StageFamily::Raster | StageFamily::Pdf | StageFamily::Media | StageFamily::PeekOnly => {
            false
        }
    }
}

/// The stamp the file at `path` has now, or `None` when it cannot be read.
pub(crate) fn stamp_of(path: &FilePath) -> Option<FileStamp> {
    std::fs::metadata(path.as_path())
        .ok()
        .map(|meta| stamp_from(&meta))
}

fn stamp_from(meta: &std::fs::Metadata) -> FileStamp {
    FileStamp {
        len: ByteLen(meta.len()),
        modified: meta
            .modified()
            .map_or(ModTime(0), ModTime::from_system_time),
    }
}
