//! 7z: the header at the end names every entry; reading it unpacks nothing.

use crate::entry::{Entry, EntryKind, EntryLimit, Holds, Listing, Seen, Tally, TallyStep};
use crate::error::ArchiveError;
use crate::limit::Limited;
use anyview_core::{ArchiveFormat, ByteLen};
use sevenz_rust::{Error, Password, SevenZReader};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const FORMAT: ArchiveFormat = ArchiveFormat::SevenZip;

/// The entries of the 7z at `path`, reading at most `budget` bytes.
pub(crate) fn list(
    path: &Path,
    limit: EntryLimit,
    budget: ByteLen,
) -> Result<Listing, ArchiveError> {
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let len = file
        .metadata()
        .map_err(|e| ArchiveError::read(path, &e))?
        .len();
    let (reader, hit) = Limited::new(BufReader::new(file), budget.0);
    let archive = match SevenZReader::new(reader, len, Password::empty()) {
        Ok(reader) => reader,
        Err(_) if hit.happened() => return Err(ArchiveError::OverBudget { allowed: budget }),
        Err(error) => return Err(error_of(&error)),
    };
    let mut tally = Tally::new(limit);
    let mut seen = Seen::All;
    for entry in &archive.archive().files {
        let kind = match entry.is_directory() {
            true => EntryKind::Directory,
            false => EntryKind::File,
        };
        let item = Entry {
            path: entry.name().to_owned(),
            kind,
            size: Some(ByteLen(entry.size())),
        };
        if tally.push(item) == TallyStep::Full {
            seen = Seen::Start;
            break;
        }
    }
    Ok(tally.finish(FORMAT, Holds::Entries, seen))
}

/// The bytes of the entry called `name`, at most `allowed` of them.
pub(crate) fn extract(path: &Path, name: &str, allowed: ByteLen) -> Result<Vec<u8>, ArchiveError> {
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let len = file
        .metadata()
        .map_err(|e| ArchiveError::read(path, &e))?
        .len();
    let mut archive = SevenZReader::new(BufReader::new(file), len, Password::empty())
        .map_err(|error| error_of(&error))?;
    let mut found: Option<Result<Vec<u8>, ArchiveError>> = None;
    let walked = archive.for_each_entries(|entry, reader| {
        if entry.name() != name {
            return Ok(true);
        }
        found = Some(read_entry(
            entry.is_directory(),
            entry.size(),
            reader,
            allowed,
        ));
        Ok(false)
    });
    match (found, walked) {
        (Some(result), _) => result,
        (None, Ok(_)) => Err(ArchiveError::NoSuchEntry {
            path: name.to_owned(),
        }),
        (None, Err(error)) => Err(error_of(&error)),
    }
}

fn read_entry(
    is_directory: bool,
    size: u64,
    reader: &mut dyn Read,
    allowed: ByteLen,
) -> Result<Vec<u8>, ArchiveError> {
    if is_directory {
        return Err(ArchiveError::NotAFile);
    }
    if size > allowed.0 {
        return Err(ArchiveError::TooLarge { allowed });
    }
    let mut bytes = Vec::new();
    reader
        .take(allowed.0.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ArchiveError::malformed(FORMAT, error))?;
    if bytes.len() as u64 > allowed.0 {
        return Err(ArchiveError::TooLarge { allowed });
    }
    Ok(bytes)
}

fn error_of(error: &Error) -> ArchiveError {
    if matches!(error, Error::PasswordRequired | Error::MaybeBadPassword(_)) {
        ArchiveError::Encrypted
    } else {
        ArchiveError::malformed(FORMAT, error)
    }
}
