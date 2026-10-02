//! Tar: a header per entry, read in place from a file or from a decompressed prefix.

use crate::entry::{Entry, EntryKind, EntryLimit, Holds, Listing, Seen, Tally, TallyStep};
use crate::error::ArchiveError;
use crate::limit::Limited;
use anyview_core::{ArchiveFormat, ByteLen};
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;
use tar::{Archive, EntryType};

/// Lists the entries of `archive` into `tally` and says how far it got; the error is the
/// parser's words for why it could not go on.
pub(crate) fn fill<R: Read + Seek>(
    archive: &mut Archive<R>,
    tally: &mut Tally,
) -> Result<Seen, String> {
    let entries = archive
        .entries_with_seek()
        .map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = kind_of(entry.header().entry_type());
        let item = Entry {
            path: path_of(&entry),
            kind,
            size: Some(ByteLen(entry.size())),
        };
        if tally.push(item) == TallyStep::Full {
            return Ok(Seen::Start);
        }
    }
    Ok(Seen::All)
}

/// An entry's path as text: bytes that are not UTF-8 are replaced.
pub(crate) fn path_of<R: Read>(entry: &tar::Entry<'_, R>) -> String {
    String::from_utf8_lossy(&entry.path_bytes()).into_owned()
}

pub(crate) fn kind_of(kind: EntryType) -> EntryKind {
    match kind {
        EntryType::Directory => EntryKind::Directory,
        EntryType::Symlink | EntryType::Link => EntryKind::Link,
        EntryType::Regular | EntryType::Continuous | EntryType::GNUSparse => EntryKind::File,
        // Devices, pipes and the extension headers the reader folds into the entry they describe
        // (the enum is open, so the rest is named by this arm).
        _ => EntryKind::Other,
    }
}

/// The start of the plain tar at `path`, reading at most `budget` bytes of headers.
pub(crate) fn list(
    path: &Path,
    limit: EntryLimit,
    budget: ByteLen,
) -> Result<Listing, ArchiveError> {
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let (reader, hit) = Limited::new(BufReader::new(file), budget.0);
    let mut tally = Tally::new(limit);
    let seen = fill(&mut Archive::new(reader), &mut tally);
    let format = ArchiveFormat::Tar;
    match seen {
        Ok(seen) => Ok(tally.finish(format, Holds::Entries, seen)),
        Err(_) if hit.happened() || !tally.is_empty() => {
            Ok(tally.finish(format, Holds::Entries, Seen::Start))
        }
        Err(reason) => Err(ArchiveError::malformed(format, reason)),
    }
}
