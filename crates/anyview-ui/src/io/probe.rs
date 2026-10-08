//! Probing: what a file is, read from its first 4 KiB (and, for a zip, its entries), and which stage shows it.

use super::error::OpenError;
use super::job::Probed;
use super::seams::FileAccess;
use crate::StageFamily;
use crate::families::family_of;
use anyview_archive::zip_entries;
use anyview_core::{
    ByteLen, FileHead, FilePath, FileStamp, ModTime, Resume, SniffStep, Source, ZipEntries,
    open_regular, sniff, sniff_folder, sniff_zip,
};
use std::io::Read;

/// The most bytes of a zip's central directory that telling what it is may read.
const ZIP_INDEX: ByteLen = ByteLen(8 * 1024 * 1024);

/// Look at the file `path` names: stat it, read its head, sniff it. Blocking.
pub(crate) fn probe(path: &FilePath) -> Result<Probed, OpenError> {
    let meta = std::fs::metadata(path.as_path()).map_err(|e| OpenError::Read(e.kind()))?;
    let stamp = stamp_from(&meta);
    let sniffed = if meta.is_dir() {
        sniff_folder()
    } else {
        let mut head = Vec::new();
        // A FIFO, device or socket is refused here, so no later read of the file can hang on it.
        open_regular(path.as_path())
            .and_then(|(file, _)| file.take(FileHead::MAX.0).read_to_end(&mut head))
            .map_err(|e| OpenError::Read(e.kind()))?;
        let name = path.file_name().ok_or(OpenError::Unrecognised)?;
        match sniff(&FileHead::new(&head), &name) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(inside) => {
                // A zip that cannot be listed is a plain archive.
                let entries = zip_entries(path, ZIP_INDEX)
                    .unwrap_or_else(|_| ZipEntries::new(Vec::<String>::new(), None));
                sniff_zip(inside, &entries)
            }
        }
    };
    let family = family_of(sniffed.kind());
    if stamp.len == ByteLen(0) && !meta.is_dir() && !holds_text(family) {
        return Err(OpenError::Empty);
    }
    Ok(Probed {
        resume: Resume::Nothing,
        access: FileAccess::Writable,
        family,
        source: Source::new(path.clone(), stamp),
        sniffed,
    })
}

/// Whether a file of `family` can be a blank document: only text can, and a picture, a document
/// or a recording of no bytes is a file that failed to be written.
fn holds_text(family: StageFamily) -> bool {
    match family {
        StageFamily::Text | StageFamily::Table | StageFamily::Tree => true,
        StageFamily::Raster
        | StageFamily::Pdf
        | StageFamily::Media
        | StageFamily::Book
        | StageFamily::PeekOnly => false,
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
