//! Listing an archive: the one entry point, dispatching on how the format is stored.

use crate::container::{Codec, Container, container};
use crate::entry::{Entry, EntryCount, EntryKind, EntryLimit, Holds, Listing, Seen, Tally};
use crate::error::ArchiveError;
use crate::{sevenz, stream, tar, zip_archive};
use anyview_core::{ArchiveFormat, ByteLen, FilePath};
use std::io::Cursor;
use std::path::Path;

/// The start of the index of the archive at `path`, whose format `format` was sniffed.
///
/// `budget` is the most bytes it reads from the file (the index of a zip, a 7z or a tar's
/// headers) or unpacks from a compressed stream; an index larger than that is
/// [`ArchiveError::OverBudget`] for a zip or a 7z, and a lower-bound count for a tar or a stream.
/// `limit` is how many entries it keeps. Blocking: run it on a worker.
pub fn list(
    path: &FilePath,
    format: ArchiveFormat,
    limit: EntryLimit,
    budget: ByteLen,
) -> Result<Listing, ArchiveError> {
    if budget.0 == 0 {
        return Err(ArchiveError::NoBudget);
    }
    let path = path.as_path();
    match container(format) {
        Container::Zip => zip_archive::list(path, format, limit, budget),
        Container::Tar => tar::list(path, limit, budget),
        Container::Compressed(codec) => compressed(path, format, codec, limit, budget),
        Container::SevenZip => sevenz::list(path, limit, budget),
    }
}

/// A compressed stream holds a tar or one file; the first bytes it unpacks to say which.
fn compressed(
    path: &Path,
    format: ArchiveFormat,
    codec: Codec,
    limit: EntryLimit,
    budget: ByteLen,
) -> Result<Listing, ArchiveError> {
    let unpacked = stream::unpack(path, format, codec, budget.0)?;
    let mut tally = Tally::new(limit);
    let mut archive = ::tar::Archive::new(Cursor::new(&unpacked.bytes[..]));
    let read = tar::fill(&mut archive, &mut tally);
    if tally.is_empty() {
        return Ok(one_file(path, format, &unpacked));
    }
    let seen = match (read, unpacked.whole) {
        (Ok(Seen::All), true) => Seen::All,
        (Ok(Seen::All | Seen::Start) | Err(_), true | false) => Seen::Start,
    };
    Ok(tally.finish(format, Holds::Entries, seen))
}

fn one_file(path: &Path, format: ArchiveFormat, unpacked: &stream::Unpacked) -> Listing {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let size = unpacked
        .whole
        .then_some(ByteLen(unpacked.bytes.len() as u64));
    Listing {
        format,
        holds: Holds::OneFile,
        entries: vec![Entry {
            path: name,
            kind: EntryKind::File,
            size,
        }],
        count: EntryCount::Exact(1),
        unpacked: size.unwrap_or(ByteLen(0)),
    }
}
