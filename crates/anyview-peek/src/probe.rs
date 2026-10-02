//! Probing: what a path is. It stats the path, reads its first 4 KiB and sniffs it, looking inside
//! a zip for what kind of zip it is; the answer is what [`peek`](crate::peek) takes.

use crate::error::PeekError;
use anyview_archive::zip_entries;
use anyview_core::{
    ByteLen, FileHead, FilePath, FileStamp, ModTime, SniffStep, Sniffed, Source, ZipEntries, sniff,
    sniff_folder, sniff_zip,
};
use std::fs::File;
use std::io::Read;

/// The most of a zip's central directory sniffing reads to tell a document from an archive; a
/// zip with a larger one is sniffed as a plain archive.
const ZIP_INDEX: ByteLen = ByteLen(256 * 1024);

/// A path that was looked at: the file as it was then, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    /// The path with the size and modification time it had when it was probed.
    pub source: Source,
    /// What sniffing made of it.
    pub sniffed: Sniffed,
}

/// Looks at `path`: a folder is a folder; a file is sniffed from its head and, when it is a zip,
/// from its entries (an unreadable zip is a plain archive, which a peek then reports on). Blocking:
/// run it on a worker.
pub fn probe(path: &FilePath) -> Result<Probed, PeekError> {
    let as_path = path.as_path();
    let fault = |error: &std::io::Error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PeekError::Missing {
                path: as_path.to_path_buf(),
            }
        } else {
            PeekError::Unreadable {
                path: as_path.to_path_buf(),
                kind: error.kind(),
            }
        }
    };
    let meta = std::fs::metadata(as_path).map_err(|e| fault(&e))?;
    let stamp = FileStamp {
        len: ByteLen(meta.len()),
        modified: meta
            .modified()
            .map_or(ModTime(0), ModTime::from_system_time),
    };
    let sniffed = if meta.is_dir() {
        sniff_folder()
    } else {
        let mut head = Vec::new();
        File::open(as_path)
            .and_then(|file| file.take(FileHead::MAX.0).read_to_end(&mut head))
            .map_err(|e| fault(&e))?;
        let name = path.file_name().ok_or(PeekError::Unreadable {
            path: as_path.to_path_buf(),
            kind: std::io::ErrorKind::InvalidInput,
        })?;
        match sniff(&FileHead::new(&head), &name) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(inside) => {
                let entries = zip_entries(path, ZIP_INDEX)
                    .unwrap_or_else(|_| ZipEntries::new(Vec::<String>::new(), None));
                sniff_zip(inside, &entries)
            }
        }
    };
    Ok(Probed {
        source: Source::new(path.clone(), stamp),
        sniffed,
    })
}
