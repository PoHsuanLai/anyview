//! Where text bytes come from: a file read in ranges, or bytes already in memory.

use crate::error::TextError;
use anyview_core::{ByteLen, Source};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::PathBuf;

/// Bytes that can be read by range, so a huge file is windowed without reading it whole. Two
/// implementations swap here: the file the viewer opened, and bytes held in memory (a peek's
/// head, a test's literal).
pub trait ByteSource {
    /// How many bytes there are.
    fn byte_len(&self) -> ByteLen;

    /// The bytes in `range`, fewer when the source ends first.
    fn read(&self, range: Range<u64>) -> Result<Vec<u8>, TextError>;
}

/// A file on disk, opened afresh for each range so no handle is held between windows.
#[derive(Debug, Clone)]
pub struct FileBytes {
    path: PathBuf,
    len: ByteLen,
}

impl FileBytes {
    /// The file `src` names, with the length it has now.
    pub fn open(src: &Source) -> Result<Self, TextError> {
        let path = src.path().as_path().to_path_buf();
        let len = std::fs::metadata(&path)
            .map_err(|e| read_error(&path, &e))?
            .len();
        Ok(FileBytes {
            path,
            len: ByteLen(len),
        })
    }
}

impl FileBytes {
    /// The first `limit` bytes of the file `src` names: a file that seems no longer than that,
    /// so a window of lines of it is the file's start and nothing is indexed past it. A file
    /// shorter than `limit` is whole.
    pub fn first(src: &Source, limit: ByteLen) -> Result<Self, TextError> {
        let whole = FileBytes::open(src)?;
        Ok(FileBytes {
            len: ByteLen(whole.len.0.min(limit.0)),
            ..whole
        })
    }
}

fn read_error(path: &std::path::Path, error: &std::io::Error) -> TextError {
    TextError::Read {
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

impl ByteSource for FileBytes {
    fn byte_len(&self) -> ByteLen {
        self.len
    }

    fn read(&self, range: Range<u64>) -> Result<Vec<u8>, TextError> {
        let end = range.end.min(self.len.0);
        let wanted = end.saturating_sub(range.start);
        let mut file = File::open(&self.path).map_err(|e| read_error(&self.path, &e))?;
        file.seek(SeekFrom::Start(range.start))
            .map_err(|e| read_error(&self.path, &e))?;
        let mut out = Vec::new();
        file.take(wanted)
            .read_to_end(&mut out)
            .map_err(|e| read_error(&self.path, &e))?;
        Ok(out)
    }
}

/// Bytes held in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldBytes(Vec<u8>);

impl HeldBytes {
    /// These bytes.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        HeldBytes(bytes.into())
    }
}

impl ByteSource for HeldBytes {
    fn byte_len(&self) -> ByteLen {
        ByteLen(self.0.len() as u64) // a usize fits a u64
    }

    fn read(&self, range: Range<u64>) -> Result<Vec<u8>, TextError> {
        let end = usize::try_from(range.end)
            .unwrap_or(usize::MAX)
            .min(self.0.len());
        let start = usize::try_from(range.start).unwrap_or(usize::MAX).min(end);
        Ok(self.0[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_of_a_file_reads_as_a_shorter_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"0123456789").unwrap();
        let src = Source::new(
            anyview_core::FilePath::new(&path).unwrap(),
            anyview_core::FileStamp {
                len: ByteLen(10),
                modified: anyview_core::ModTime(0),
            },
        );
        // name, limit, length it reports, what a read of everything gives
        const CASES: &[(&str, u64, u64, &[u8])] = &[
            ("shorter than the file", 4, 4, b"0123"),
            ("exactly the file", 10, 10, b"0123456789"),
            ("longer than the file", 99, 10, b"0123456789"),
        ];
        for (name, limit, len, bytes) in CASES {
            let start = FileBytes::first(&src, ByteLen(*limit)).unwrap();
            assert_eq!(start.byte_len(), ByteLen(*len), "{name}: length");
            assert_eq!(start.read(0..99).unwrap(), *bytes, "{name}: bytes");
        }
    }

    #[test]
    fn held_bytes_clip_a_range_to_what_exists() {
        // name, start, end, bytes
        const CASES: &[(&str, u64, u64, &[u8])] = &[
            ("inside", 1, 3, b"bc"),
            ("whole", 0, 4, b"abcd"),
            ("past the end", 2, 99, b"cd"),
            ("starts past the end", 9, 12, b""),
            ("empty", 2, 2, b""),
            ("reversed", 3, 1, b""),
        ];
        let held = HeldBytes::new(*b"abcd");
        for (name, start, end, want) in CASES {
            assert_eq!(held.read(*start..*end).unwrap(), *want, "{name}");
        }
        assert_eq!(held.byte_len(), ByteLen(4));
    }
}
