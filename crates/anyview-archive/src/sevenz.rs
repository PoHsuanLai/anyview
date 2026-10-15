//! 7z: the header at the end names every entry; reading it unpacks nothing.

use crate::entry::{Entry, EntryKind, EntryLimit, Holds, Listing, Seen, Tally, TallyStep};
use crate::error::ArchiveError;
use crate::extract::ExtractLimits;
use crate::limit::{Limited, pull};
use crate::sevenz_header::{self, Fault};
use anyview_core::work::Interrupt;
use anyview_core::{ArchiveFormat, ByteLen, Input, ReadAtStream};
use sevenz_rust2::{ArchiveEntry, ArchiveReader, Error, Password};
use std::io::{BufReader, Read};

const FORMAT: ArchiveFormat = ArchiveFormat::SevenZip;

/// The entries of the 7z `src`, reading at most `budget` bytes.
pub(crate) fn list(
    src: &Input,
    limit: EntryLimit,
    budget: ByteLen,
    interrupt: &dyn Interrupt,
) -> Result<Listing, ArchiveError> {
    let file = open_checked(src, budget)?;
    let (reader, hit) = Limited::new(BufReader::new(file), budget.0);
    let archive = match ArchiveReader::new(reader, Password::empty()) {
        Ok(reader) => reader,
        Err(_) if hit.happened() => return Err(ArchiveError::OverBudget { allowed: budget }),
        Err(error) => return Err(error_of(&error)),
    };
    let mut tally = Tally::new(limit);
    let mut seen = Seen::All;
    for entry in &archive.archive().files {
        if interrupt.interrupted() {
            if tally.is_empty() {
                return Err(ArchiveError::Interrupted);
            }
            seen = Seen::Start;
            break;
        }
        let kind = kind_of(entry.is_directory());
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

/// The 7z `src`, open, once its header has been checked against the file and
/// `budget`: the crate sizes buffers from what the header says, so a header that lies must not
/// reach it.
fn open_checked(src: &Input, budget: ByteLen) -> Result<ReadAtStream, ArchiveError> {
    let mut file = ArchiveError::open(src)?;
    // The length the reader really has, not the one the source claims.
    let len = src.bytes().len().0;
    match sevenz_header::check(&mut file, len, budget.0) {
        Ok(()) => Ok(file),
        Err(Fault::Large) => Err(ArchiveError::OverBudget { allowed: budget }),
        Err(Fault::Broken(reason)) => Err(ArchiveError::malformed(FORMAT, reason)),
    }
}

/// The bytes of the entry called `name`, at most `limits.entry` of them. A solid block is
/// decoded from its start, so the entries that have to be decoded before this one (all of them
/// that come first in the header) may add up to at most `limits.scanned`; the decoder cannot be
/// asked to stop in the middle of that, so it is held to the sizes the header states beforehand.
pub(crate) fn extract(
    src: &Input,
    name: &str,
    limits: ExtractLimits,
    interrupt: &dyn Interrupt,
) -> Result<Vec<u8>, ArchiveError> {
    let allowed = limits.entry;
    let file = open_checked(src, ByteLen(u64::MAX))?;
    let mut archive = ArchiveReader::new(BufReader::new(file), Password::empty())
        .map_err(|error| error_of(&error))?;
    let to_decode = decoded_before(&archive.archive().files, name);
    if to_decode > limits.scanned.0 {
        return Err(ArchiveError::TooLarge {
            allowed: limits.scanned,
        });
    }
    let mut found: Option<Result<Vec<u8>, ArchiveError>> = None;
    let walked = archive.for_each_entries(|entry, reader| {
        if interrupt.interrupted() {
            found = Some(Err(ArchiveError::Interrupted));
            return Ok(false);
        }
        if entry.name() != name {
            return Ok(true);
        }
        found = Some(read_entry(
            kind_of(entry.is_directory()),
            entry.size(),
            reader,
            allowed,
            interrupt,
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

/// How many bytes are decoded before the entry `name` is reached, by the sizes the header
/// states: the entries with data up to it, or every one when it is an empty entry (those are
/// visited last) or is not there.
fn decoded_before(files: &[ArchiveEntry], name: &str) -> u64 {
    let end = match files.iter().position(|file| file.name() == name) {
        Some(at) if files[at].has_stream() => at + 1,
        _ => files.len(),
    };
    files[..end]
        .iter()
        .filter(|file| file.has_stream())
        .fold(0, |sum: u64, file| sum.saturating_add(file.size()))
}

/// A directory flag of the header as the kind it names.
fn kind_of(is_directory: bool) -> EntryKind {
    if is_directory {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn read_entry(
    kind: EntryKind,
    size: u64,
    reader: &mut dyn Read,
    allowed: ByteLen,
    interrupt: &dyn Interrupt,
) -> Result<Vec<u8>, ArchiveError> {
    if kind != EntryKind::File {
        return Err(ArchiveError::NotAFile);
    }
    if size > allowed.0 {
        return Err(ArchiveError::TooLarge { allowed });
    }
    let pulled = pull(reader, allowed.0, interrupt)
        .map_err(|error| ArchiveError::malformed(FORMAT, error))?;
    if pulled.interrupted {
        return Err(ArchiveError::Interrupted);
    }
    let bytes = pulled.bytes;
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
