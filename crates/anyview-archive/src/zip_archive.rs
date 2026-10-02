//! Zip, jar and apk: the central directory lists every entry without unpacking any.

use crate::entry::{
    COUNT_LIMIT, Entry, EntryKind, EntryLimit, Holds, Listing, Seen, Tally, TallyStep,
};
use crate::error::ArchiveError;
use crate::limit::Limited;
use anyview_core::{ArchiveFormat, ByteLen, FilePath, ZipEntries};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;
use zip::result::ZipError;

const SYMLINK: u32 = 0o120_000;
const TYPE_MASK: u32 = 0o170_000;

/// The start of the central directory of the zip at `path`, reading at most `budget` bytes.
pub(crate) fn list(
    path: &Path,
    format: ArchiveFormat,
    limit: EntryLimit,
    budget: ByteLen,
) -> Result<Listing, ArchiveError> {
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let (reader, hit) = Limited::new(BufReader::new(file), budget.0);
    let mut archive = match ZipArchive::new(reader) {
        Ok(archive) => archive,
        Err(_) if hit.happened() => return Err(ArchiveError::OverBudget { allowed: budget }),
        Err(error) => return Err(ArchiveError::malformed(format, error)),
    };
    let mut tally = Tally::new(limit);
    let mut seen = Seen::All;
    for index in 0..archive.len() {
        let entry = match archive.by_index_raw(index) {
            Ok(file) => Entry {
                path: file.name().to_owned(),
                kind: kind_of(file.is_dir(), file.unix_mode()),
                size: Some(ByteLen(file.size())),
            },
            Err(_) if hit.happened() => {
                seen = Seen::Start;
                break;
            }
            Err(error) => return Err(ArchiveError::malformed(format, error)),
        };
        if tally.push(entry) == TallyStep::Full {
            seen = Seen::Start;
            break;
        }
    }
    Ok(tally.finish(format, Holds::Entries, seen))
}

fn kind_of(is_dir: bool, mode: Option<u32>) -> EntryKind {
    match (is_dir, mode.map(|mode| mode & TYPE_MASK)) {
        (true, _) => EntryKind::Directory,
        (false, Some(SYMLINK)) => EntryKind::Link,
        (false, _) => EntryKind::File,
    }
}

/// The bytes of the entry called `name`, at most `allowed` of them.
pub(crate) fn extract(
    path: &Path,
    format: ArchiveFormat,
    name: &str,
    allowed: ByteLen,
) -> Result<Vec<u8>, ArchiveError> {
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let mut archive = ZipArchive::new(BufReader::new(file))
        .map_err(|error| ArchiveError::malformed(format, error))?;
    let mut entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => {
            return Err(ArchiveError::NoSuchEntry {
                path: name.to_owned(),
            });
        }
        Err(ZipError::UnsupportedArchive(reason)) if reason.contains("assword") => {
            return Err(ArchiveError::Encrypted);
        }
        Err(error) => return Err(ArchiveError::malformed(format, error)),
    };
    if entry.is_dir() {
        return Err(ArchiveError::NotAFile);
    }
    if entry.size() > allowed.0 {
        return Err(ArchiveError::TooLarge { allowed });
    }
    let mut bytes = Vec::new();
    (&mut entry)
        .take(allowed.0.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ArchiveError::malformed(format, error))?;
    if bytes.len() as u64 > allowed.0 {
        return Err(ArchiveError::TooLarge { allowed });
    }
    Ok(bytes)
}

/// How much of a `mimetype` entry sniffing reads.
const MIMETYPE_BYTES: u64 = 128;

/// What sniffing needs of the zip at `path`: every entry's name (as many as the budget lets the
/// central directory be read for) and the start of its `mimetype` entry when it has one.
///
/// A zip whose directory is larger than `budget` is [`ArchiveError::OverBudget`]; one that does not
/// parse is [`ArchiveError::Malformed`].
pub fn zip_entries(path: &FilePath, budget: ByteLen) -> Result<ZipEntries, ArchiveError> {
    let format = ArchiveFormat::Zip;
    let path = path.as_path();
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let (reader, hit) = Limited::new(BufReader::new(file), budget.0);
    let mut archive = match ZipArchive::new(reader) {
        Ok(archive) => archive,
        Err(_) if hit.happened() => return Err(ArchiveError::OverBudget { allowed: budget }),
        Err(error) => return Err(ArchiveError::malformed(format, error)),
    };
    let names: Vec<String> = archive
        .file_names()
        .take(COUNT_LIMIT as usize)
        .map(str::to_owned)
        .collect();
    // A `mimetype` that cannot be read is no `mimetype`: the names still say what the zip is.
    let mimetype = archive.by_name("mimetype").ok().and_then(|entry| {
        let mut head = Vec::new();
        entry.take(MIMETYPE_BYTES).read_to_end(&mut head).ok()?;
        Some(head)
    });
    Ok(ZipEntries::new(names, mimetype.as_deref()))
}
