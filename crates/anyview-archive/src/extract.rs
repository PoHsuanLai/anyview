//! Extracting one entry of an archive into memory, so it can be opened as a file of its own.

use crate::container::{Container, container};
use crate::entry::{EntryKind, Seen};
use crate::error::ArchiveError;
use crate::limit::pull;
use crate::{sevenz, stream, zip_archive};
use anyview_core::work::Interrupt;
use anyview_core::{ArchiveFormat, ByteLen, Deadline, Input};
use std::io::{BufReader, Cursor, Read};
use tar::Archive;

/// What an extraction may spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ExtractLimits {
    /// The largest entry it returns.
    pub entry: ByteLen,
    /// The most of a compressed stream, or of a 7z's earlier entries, it unpacks while looking
    /// for the entry.
    pub scanned: ByteLen,
}

impl ExtractLimits {
    /// Limits that return an entry of at most `entry` bytes and look at most `scanned` bytes of
    /// a compressed stream, or of a 7z's earlier entries, for it.
    #[must_use]
    pub const fn new(entry: ByteLen, scanned: ByteLen) -> Self {
        ExtractLimits { entry, scanned }
    }

    /// These limits, returning an entry of at most `entry` bytes.
    #[must_use]
    pub const fn with_entry(self, entry: ByteLen) -> Self {
        ExtractLimits { entry, ..self }
    }

    /// These limits, looking at most `scanned` bytes for the entry.
    #[must_use]
    pub const fn with_scanned(self, scanned: ByteLen) -> Self {
        ExtractLimits { scanned, ..self }
    }
}

/// The bytes of the entry `entry` of the archive `src` (a path, or any source a host injects), whose format `format` was sniffed.
/// Blocking: run it on a worker. A folder or a link is [`ArchiveError::NotAFile`].
pub fn extract(
    src: impl Into<Input>,
    format: ArchiveFormat,
    entry: &str,
    limits: ExtractLimits,
) -> Result<Vec<u8>, ArchiveError> {
    extract_until(src, format, entry, limits, &Deadline::NEVER)
}

/// [`extract`], ending with [`ArchiveError::Interrupted`] as soon as `interrupt` says so: it is
/// asked once per entry looked at and once per chunk of an entry or a compressed stream unpacked.
/// A 7z's decoder cannot be stopped inside an entry it passes over, so the entries it has to
/// decode before the wanted one are held to `limits.scanned` beforehand, by the sizes the header
/// states. A part of an entry is never returned.
pub fn extract_until(
    src: impl Into<Input>,
    format: ArchiveFormat,
    entry: &str,
    limits: ExtractLimits,
    interrupt: &dyn Interrupt,
) -> Result<Vec<u8>, ArchiveError> {
    let src = &src.into();
    match container(format) {
        Container::Zip => zip_archive::extract(src, format, entry, limits.entry, interrupt),
        Container::SevenZip => sevenz::extract(src, entry, limits, interrupt),
        Container::Tar => {
            let file = ArchiveError::open(src)?;
            tar_entry(
                &mut Archive::new(BufReader::new(file)),
                format,
                entry,
                limits.entry,
                interrupt,
            )
        }
        Container::Compressed(codec) => {
            let unpacked = stream::unpack(src, format, codec, limits.scanned.0, interrupt)?;
            if unpacked.interrupted {
                return Err(ArchiveError::Interrupted);
            }
            let mut archive = Archive::new(Cursor::new(&unpacked.bytes[..]));
            match tar_entry(&mut archive, format, entry, limits.entry, interrupt) {
                Ok(bytes) => Ok(bytes),
                Err(ArchiveError::NoSuchEntry { .. }) if unpacked.seen == Seen::Start => {
                    Err(ArchiveError::TooLarge {
                        allowed: limits.scanned,
                    })
                }
                Err(error) => single_file(src, entry, unpacked, limits, error),
            }
        }
    }
}

/// The one file a compressed stream holds, when it is not a tar and `entry` names it.
fn single_file(
    src: &Input,
    entry: &str,
    unpacked: stream::Unpacked,
    limits: ExtractLimits,
    not_tar: ArchiveError,
) -> Result<Vec<u8>, ArchiveError> {
    let name = crate::list::stem_of(src);
    match (name, not_tar) {
        (Some(name), ArchiveError::Malformed { .. } | ArchiveError::NoSuchEntry { .. })
            if name == entry =>
        {
            if unpacked.seen == Seen::All && unpacked.bytes.len() as u64 <= limits.entry.0 {
                Ok(unpacked.bytes)
            } else {
                Err(ArchiveError::TooLarge {
                    allowed: limits.entry,
                })
            }
        }
        (_, error) => Err(error),
    }
}

fn tar_entry<R: Read + std::io::Seek>(
    archive: &mut Archive<R>,
    format: ArchiveFormat,
    wanted: &str,
    allowed: ByteLen,
    interrupt: &dyn Interrupt,
) -> Result<Vec<u8>, ArchiveError> {
    let entries = archive
        .entries_with_seek()
        .map_err(|error| ArchiveError::malformed(format, error))?;
    for entry in entries {
        if interrupt.interrupted() {
            return Err(ArchiveError::Interrupted);
        }
        let mut entry = entry.map_err(|error| ArchiveError::malformed(format, error))?;
        if crate::tar::path_of(&entry) != wanted {
            continue;
        }
        if crate::tar::kind_of(entry.header().entry_type()) != EntryKind::File {
            return Err(ArchiveError::NotAFile);
        }
        if entry.size() > allowed.0 {
            return Err(ArchiveError::TooLarge { allowed });
        }
        let pulled = pull(&mut entry, allowed.0, interrupt)
            .map_err(|error| ArchiveError::malformed(format, error))?;
        if pulled.interrupted {
            return Err(ArchiveError::Interrupted);
        }
        return Ok(pulled.bytes);
    }
    Err(ArchiveError::NoSuchEntry {
        path: wanted.to_owned(),
    })
}
