//! Compressed streams: gzip, bzip2, xz and Zstandard unpacked up to a byte cap, into memory. What
//! the prefix holds (a tar, or one file) is the caller's to say.

use crate::container::Codec;
use crate::entry::Seen;
use crate::error::ArchiveError;
use crate::limit::pull;
use anyview_core::work::Interrupt;
use anyview_core::{ArchiveFormat, Input};
use std::io::{BufReader, Read};

/// The start of an unpacked stream.
#[derive(Debug)]
pub(crate) struct Unpacked {
    /// At most the cap's bytes.
    pub bytes: Vec<u8>,
    /// `All` when the stream ended inside the cap, so `bytes` is all of it.
    pub seen: Seen,
    /// Whether the unpacking was told to end before the cap or the stream's end.
    pub interrupted: bool,
}

/// The first `cap` bytes of what the stream `src` unpacks to, fewer when `interrupt` says to end.
pub(crate) fn unpack(
    src: &Input,
    format: ArchiveFormat,
    codec: Codec,
    cap: u64,
    interrupt: &dyn Interrupt,
) -> Result<Unpacked, ArchiveError> {
    let file = ArchiveError::open(src)?;
    let broken = |error: &dyn std::fmt::Display| ArchiveError::malformed(format, error);
    let source = BufReader::new(file);
    match codec {
        Codec::Gzip => read_capped(
            flate2::read::MultiGzDecoder::new(source),
            cap,
            format,
            interrupt,
        ),
        Codec::Bzip2 => read_capped(
            bzip2::read::MultiBzDecoder::new(source),
            cap,
            format,
            interrupt,
        ),
        Codec::Zstd => {
            let decoder =
                ruzstd::decoding::StreamingDecoder::new(source).map_err(|e| broken(&e))?;
            read_capped(decoder, cap, format, interrupt)
        }
        // Streamed block by block, so a block of a hundred gigabytes of zeros costs the window
        // the decoder keeps, not the block.
        Codec::Xz => read_capped(
            lzma_rust2::XzReader::new(source, false),
            cap,
            format,
            interrupt,
        ),
    }
}

fn read_capped(
    mut decoder: impl Read,
    cap: u64,
    format: ArchiveFormat,
    interrupt: &dyn Interrupt,
) -> Result<Unpacked, ArchiveError> {
    let pulled = pull(&mut decoder, cap, interrupt)
        .map_err(|error| ArchiveError::malformed(format, error))?;
    let mut bytes = pulled.bytes;
    let seen = if bytes.len() as u64 <= cap && !pulled.interrupted {
        Seen::All
    } else {
        bytes.truncate(usize::try_from(cap).unwrap_or(usize::MAX));
        Seen::Start
    };
    Ok(Unpacked {
        bytes,
        seen,
        interrupted: pulled.interrupted,
    })
}
