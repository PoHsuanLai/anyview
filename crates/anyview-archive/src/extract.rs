//! Extracting one entry of an archive into memory, so it can be opened as a file of its own.

use crate::container::{Container, container};
use crate::entry::EntryKind;
use crate::error::ArchiveError;
use crate::{sevenz, stream, zip_archive};
use anyview_core::{ArchiveFormat, ByteLen, FilePath};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use tar::Archive;

/// What an extraction may spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtractLimits {
    /// The largest entry it returns.
    pub entry: ByteLen,
    /// The most of a compressed stream it unpacks while looking for the entry.
    pub scanned: ByteLen,
}

/// The bytes of the entry `entry` of the archive at `path`, whose format `format` was sniffed.
/// Blocking: run it on a worker. A folder or a link is [`ArchiveError::NotAFile`].
pub fn extract(
    path: &FilePath,
    format: ArchiveFormat,
    entry: &str,
    limits: ExtractLimits,
) -> Result<Vec<u8>, ArchiveError> {
    let path = path.as_path();
    match container(format) {
        Container::Zip => zip_archive::extract(path, format, entry, limits.entry),
        Container::SevenZip => sevenz::extract(path, entry, limits.entry),
        Container::Tar => {
            let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
            tar_entry(
                &mut Archive::new(BufReader::new(file)),
                format,
                entry,
                limits.entry,
            )
        }
        Container::Compressed(codec) => {
            let unpacked = stream::unpack(path, format, codec, limits.scanned.0)?;
            let mut archive = Archive::new(Cursor::new(&unpacked.bytes[..]));
            match tar_entry(&mut archive, format, entry, limits.entry) {
                Ok(bytes) => Ok(bytes),
                Err(ArchiveError::NoSuchEntry { .. }) if !unpacked.whole => {
                    Err(ArchiveError::TooLarge {
                        allowed: limits.scanned,
                    })
                }
                Err(error) => single_file(path, entry, unpacked, limits, error),
            }
        }
    }
}

/// The one file a compressed stream holds, when it is not a tar and `entry` names it.
fn single_file(
    path: &std::path::Path,
    entry: &str,
    unpacked: stream::Unpacked,
    limits: ExtractLimits,
    not_tar: ArchiveError,
) -> Result<Vec<u8>, ArchiveError> {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned());
    match (name, not_tar) {
        (Some(name), ArchiveError::Malformed { .. } | ArchiveError::NoSuchEntry { .. })
            if name == entry =>
        {
            if unpacked.whole && unpacked.bytes.len() as u64 <= limits.entry.0 {
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
) -> Result<Vec<u8>, ArchiveError> {
    let entries = archive
        .entries_with_seek()
        .map_err(|error| ArchiveError::malformed(format, error))?;
    for entry in entries {
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
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| ArchiveError::malformed(format, error))?;
        return Ok(bytes);
    }
    Err(ArchiveError::NoSuchEntry {
        path: wanted.to_owned(),
    })
}
