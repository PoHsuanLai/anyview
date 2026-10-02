//! Probing: what a file is, read from its first 4 KiB, and which stage shows it.

use super::error::OpenError;
use super::job::Probed;
use crate::families::family_of;
use anyview_core::{
    ByteLen, FileHead, FilePath, FileStamp, ModTime, Resume, SniffStep, Source, sniff, sniff_folder,
};
use std::fs::File;
use std::io::Read;

/// Look at the file `path` names: stat it, read its head, sniff it. Blocking.
pub(crate) fn probe(path: &FilePath) -> Result<Probed, OpenError> {
    let meta = std::fs::metadata(path.as_path()).map_err(|e| OpenError::Read(e.kind()))?;
    let stamp = stamp_from(&meta);
    let sniffed = if meta.is_dir() {
        sniff_folder()
    } else {
        let mut head = Vec::new();
        File::open(path.as_path())
            .and_then(|file| file.take(FileHead::MAX.0).read_to_end(&mut head))
            .map_err(|e| OpenError::Read(e.kind()))?;
        let name = path.file_name().ok_or(OpenError::Unrecognised)?;
        match sniff(&FileHead::new(&head), &name) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(_) => return Err(OpenError::Unrecognised),
        }
    };
    Ok(Probed {
        resume: Resume::Nothing,
        family: family_of(sniffed.kind()),
        source: Source::new(path.clone(), stamp),
        sniffed,
    })
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
