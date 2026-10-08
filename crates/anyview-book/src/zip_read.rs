//! The two reads a book makes of its zip, both through `anyview-archive`.

use crate::error::BookError;
use anyview_archive::{EntryKind, EntryLimit, ExtractLimits, extract, list};
use anyview_core::{ArchiveFormat, ByteLen, Input};

/// The most bytes of the zip's index a book reads.
const INDEX_BUDGET: ByteLen = ByteLen(64 * 1024 * 1024);

/// The most entries a book keeps from the index.
const INDEX_ENTRIES: EntryLimit = EntryLimit(100_000);

/// The names of the files in the zip at `input`, in the zip's own order.
pub(crate) fn file_names(input: &Input) -> Result<Vec<String>, BookError> {
    let listing = list(input, ArchiveFormat::Zip, INDEX_ENTRIES, INDEX_BUDGET)?;
    Ok(listing
        .entries
        .into_iter()
        .filter(|entry| entry.kind == EntryKind::File)
        .map(|entry| entry.path)
        .collect())
}

/// The bytes of the entry `name` of the zip at `input`, at most `allowed` of them.
pub(crate) fn read(input: &Input, name: &str, allowed: ByteLen) -> Result<Vec<u8>, BookError> {
    let limits = ExtractLimits {
        entry: allowed,
        scanned: allowed,
    };
    Ok(extract(input, ArchiveFormat::Zip, name, limits)?)
}
