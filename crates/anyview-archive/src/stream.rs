//! Compressed streams: gzip, bzip2, xz and Zstandard unpacked up to a byte cap, into memory. What
//! the prefix holds (a tar, or one file) is the caller's to say.

use crate::container::Codec;
use crate::error::ArchiveError;
use anyview_core::ArchiveFormat;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// The start of an unpacked stream.
#[derive(Debug)]
pub(crate) struct Unpacked {
    /// At most the cap's bytes.
    pub bytes: Vec<u8>,
    /// Whether the stream ended inside the cap, so `bytes` is all of it.
    pub whole: bool,
}

/// The first `cap` bytes of what the stream at `path` unpacks to.
pub(crate) fn unpack(
    path: &Path,
    format: ArchiveFormat,
    codec: Codec,
    cap: u64,
) -> Result<Unpacked, ArchiveError> {
    let file = File::open(path).map_err(|e| ArchiveError::read(path, &e))?;
    let broken = |error: &dyn std::fmt::Display| ArchiveError::malformed(format, error);
    let source = BufReader::new(file);
    match codec {
        Codec::Gzip => read_capped(flate2::read::MultiGzDecoder::new(source), cap, format),
        Codec::Bzip2 => read_capped(bzip2::read::MultiBzDecoder::new(source), cap, format),
        Codec::Zstd => {
            let decoder =
                ruzstd::decoding::StreamingDecoder::new(source).map_err(|e| broken(&e))?;
            read_capped(decoder, cap, format)
        }
        // Streamed block by block, so a block of a hundred gigabytes of zeros costs the window
        // the decoder keeps, not the block.
        Codec::Xz => read_capped(lzma_rust2::XzReader::new(source, false), cap, format),
    }
}

fn read_capped(
    decoder: impl Read,
    cap: u64,
    format: ArchiveFormat,
) -> Result<Unpacked, ArchiveError> {
    let mut bytes = Vec::new();
    decoder
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ArchiveError::malformed(format, error))?;
    let whole = bytes.len() as u64 <= cap;
    if !whole {
        bytes.truncate(usize::try_from(cap).unwrap_or(usize::MAX));
    }
    Ok(Unpacked { bytes, whole })
}
