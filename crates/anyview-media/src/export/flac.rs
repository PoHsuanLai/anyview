//! A FLAC file's stream header. The encoder learns how many samples it wrote only at the end and
//! hands that over in a packet with no audio in it, which the libav binding will not write, so the
//! total is put in the file's header afterwards. Without it a player cannot tell how long the file
//! is until it has read all of it.

use crate::MediaError;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Where the 64 bits holding the sample rate, channels, bit depth and total sample count start: after
/// the `fLaC` marker, the block header and the first 10 bytes of the stream info.
const PACKED_AT: u64 = 4 + 4 + 10;

/// The low 36 bits of those 64: the total number of samples.
const TOTAL_MASK: u64 = (1 << 36) - 1;

/// Say in the stream info of the FLAC file at `path` that it holds `samples` samples a channel.
pub(super) fn set_total(path: &Path, samples: u64) -> Result<(), MediaError> {
    let io = |op: &'static str| {
        move |error: std::io::Error| MediaError::Io {
            op,
            path: path.to_path_buf(),
            kind: error.kind(),
        }
    };
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(io("open"))?;
    file.seek(SeekFrom::Start(PACKED_AT)).map_err(io("read"))?;
    let mut packed = [0_u8; 8];
    file.read_exact(&mut packed).map_err(io("read"))?;
    let patched = (u64::from_be_bytes(packed) & !TOTAL_MASK) | (samples & TOTAL_MASK);
    file.seek(SeekFrom::Start(PACKED_AT)).map_err(io("write"))?;
    file.write_all(&patched.to_be_bytes()).map_err(io("write"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_total_replaces_the_low_bits_and_keeps_the_rate_and_depth() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.flac");
        let mut bytes = b"fLaC\x00\x00\x00\x22".to_vec();
        bytes.extend([0_u8; 10]);
        // 8000 Hz, mono, 16 bits, and no total yet.
        let packed: u64 = (8000 << 44) | (15 << 36);
        bytes.extend(packed.to_be_bytes());
        bytes.extend([0_u8; 16]);
        std::fs::write(&path, &bytes).unwrap();
        set_total(&path, 16_000).unwrap();
        let after = std::fs::read(&path).unwrap();
        let read = u64::from_be_bytes(after[18..26].try_into().unwrap());
        assert_eq!(read & TOTAL_MASK, 16_000);
        assert_eq!(read >> 36, packed >> 36);
        assert_eq!(after.len(), bytes.len());
    }
}
