//! Probing: what a file is. It reads the file's first 4 KiB and sniffs it, looking inside a zip
//! for what kind of zip it is; the answer is what [`peek`](crate::peek) takes.

use crate::error::PeekError;
use anyview_archive::zip_entries;
use anyview_core::{
    ByteLen, FileHead, Input, SniffStep, Sniffed, ZipEntries, sniff, sniff_folder, sniff_zip,
};
use std::io::{Error, ErrorKind};

/// The most of a zip's central directory sniffing reads to tell a document from an archive; a
/// zip with a larger one is sniffed as a plain archive.
const ZIP_INDEX: ByteLen = ByteLen(256 * 1024);

/// A file that was looked at: the file as it was then, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    /// The file, with the name and stamp it had when it was probed.
    pub input: Input,
    /// What sniffing made of it.
    pub sniffed: Sniffed,
}

/// Looks at `src`, a path (`&FilePath`, `&Source`) or any bytes a host injects: a folder is a
/// folder; a file is sniffed from its head and, when it is a zip, from its entries (an unreadable
/// zip is a plain archive, which a peek then reports on). Blocking: run it on a worker.
pub fn probe(src: impl Into<Input>) -> Result<Probed, PeekError> {
    let input = src.into();
    let fault = |error: &Error| {
        let path = input.label();
        if error.kind() == ErrorKind::NotFound {
            PeekError::Missing { path }
        } else {
            PeekError::Unreadable {
                path,
                kind: error.kind(),
            }
        }
    };
    let is_folder = match input.path() {
        Some(path) => std::fs::metadata(path.as_path())
            .map_err(|e| fault(&e))?
            .is_dir(),
        None => false,
    };
    let sniffed = if is_folder {
        sniff_folder()
    } else {
        // A FIFO, device or socket is refused here, so no later read of the file can hang on it.
        let head = input
            .bytes()
            .read_range(0..FileHead::MAX.0)
            .map_err(|e| fault(&e))?;
        match sniff(&FileHead::new(&head), input.name()) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(inside) => {
                let entries = zip_entries(&input, ZIP_INDEX)
                    .unwrap_or_else(|_| ZipEntries::new(Vec::<String>::new(), None));
                sniff_zip(inside, &entries)
            }
        }
    };
    Ok(Probed { input, sniffed })
}
