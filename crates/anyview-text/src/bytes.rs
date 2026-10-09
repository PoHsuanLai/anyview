//! Where text bytes come from: a file read in ranges (any other source is `anyview_core::ReadAt`).

use crate::error::TextError;
use anyview_core::{ByteLen, ReadAt, Source};
use anyview_fs::OpenFile;
use std::io::Result as IoResult;
use std::ops::Range;
use std::path::{Path, PathBuf};

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

impl ReadAt for FileBytes {
    fn len(&self) -> ByteLen {
        self.len
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> IoResult<usize> {
        // Opened afresh for each read; a read past the length the source was given reads nothing.
        let room = self.len.0.saturating_sub(offset);
        let take = buf.len().min(usize::try_from(room).unwrap_or(usize::MAX));
        let file = OpenFile::open(&self.path)?;
        file.read_at(offset, &mut buf[..take])
    }
}

/// `range` of `source`, as text reads it: a failure names no path, the source knows none.
pub(crate) fn read_range(
    source: &(impl ReadAt + ?Sized),
    range: Range<u64>,
) -> Result<Vec<u8>, TextError> {
    source
        .read_range(range)
        .map_err(|e| read_error(Path::new(""), &e))
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
            assert_eq!(start.len(), ByteLen(*len), "{name}: length");
            assert_eq!(start.read_range(0..99).unwrap(), *bytes, "{name}: bytes");
        }
    }
}
