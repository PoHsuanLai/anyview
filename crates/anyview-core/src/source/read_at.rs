//! `ReadAt`: bytes read by offset, so a huge file is windowed without reading it whole and a peek
//! reads a path, a database blob or bytes in memory alike.

use std::fmt::Debug;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom};
use std::ops::Range;
use std::sync::Arc;

use super::ByteLen;

/// Bytes that can be read at an offset, from any thread. The built-in implementations are a byte
/// slice, a `Vec`, an `Arc<[u8]>` and an open `File`; a host implements it for anything else (a
/// blob in a database, a spool, an archive entry). A source must end: a FIFO or an endless stream
/// is not one. Callers hold what they take to a budget; a source need not.
pub trait ReadAt: Debug + Send + Sync {
    /// How many bytes there are now.
    fn len(&self) -> ByteLen;

    /// Reads into `buf` from `offset`, as [`std::io::Read::read`] does: it may fill less than
    /// `buf`, and gives `0` at the end.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// Whether there are no bytes.
    fn is_empty(&self) -> bool {
        self.len().0 == 0
    }

    /// The bytes in `range`, fewer when the source ends first.
    fn read_range(&self, range: Range<u64>) -> Result<Vec<u8>> {
        let wanted = range.end.saturating_sub(range.start).min(self.len().0);
        if wanted == 0 {
            // Nothing to read, but a source that cannot be read says so (a file that is not
            // there reports a length of none).
            self.read_at(range.start, &mut [])?;
            return Ok(Vec::new());
        }
        let mut out = vec![0; usize::try_from(wanted).map_err(|_| ErrorKind::OutOfMemory)?];
        let mut filled = 0;
        while filled < out.len() {
            match self.read_at(range.start + filled as u64, &mut out[filled..]) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        out.truncate(filled);
        Ok(out)
    }
}

/// The start of `bytes` from `offset`, copied into `buf`.
fn from_slice(bytes: &[u8], offset: u64, buf: &mut [u8]) -> usize {
    let start = usize::try_from(offset).map_or(bytes.len(), |at| at.min(bytes.len()));
    let count = buf.len().min(bytes.len() - start);
    buf[..count].copy_from_slice(&bytes[start..start + count]);
    count
}

impl ReadAt for [u8] {
    fn len(&self) -> ByteLen {
        ByteLen(<[u8]>::len(self) as u64) // a usize fits a u64
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        Ok(from_slice(self, offset, buf))
    }
}

impl ReadAt for Vec<u8> {
    fn len(&self) -> ByteLen {
        ReadAt::len(self.as_slice())
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.as_slice().read_at(offset, buf)
    }
}

impl<T: ReadAt + ?Sized> ReadAt for Arc<T> {
    fn len(&self) -> ByteLen {
        (**self).len()
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        (**self).read_at(offset, buf)
    }
}

impl ReadAt for File {
    fn len(&self) -> ByteLen {
        ByteLen(self.metadata().map_or(0, |meta| meta.len()))
    }

    #[cfg(unix)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        std::os::unix::fs::FileExt::read_at(self, buf, offset)
    }

    #[cfg(windows)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        std::os::windows::fs::FileExt::seek_read(self, buf, offset)
    }
}

/// A `Read + Seek` over a [`ReadAt`], for the zip, tar and EPUB libraries, which read a stream.
#[derive(Clone)]
pub struct ReadAtStream {
    bytes: Arc<dyn ReadAt>,
    at: u64,
}

impl ReadAtStream {
    /// A stream over `bytes`, at their start.
    pub fn new(bytes: Arc<dyn ReadAt>) -> Self {
        ReadAtStream { bytes, at: 0 }
    }
}

impl std::fmt::Debug for ReadAtStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadAtStream")
            .field("at", &self.at)
            .finish_non_exhaustive()
    }
}

impl Read for ReadAtStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let read = self.bytes.read_at(self.at, buf)?;
        self.at += read as u64; // a usize fits a u64
        Ok(read)
    }
}

impl Seek for ReadAtStream {
    fn seek(&mut self, to: SeekFrom) -> Result<u64> {
        let (base, delta) = match to {
            SeekFrom::Start(at) => (0, i128::from(at)),
            SeekFrom::Current(delta) => (self.at, i128::from(delta)),
            SeekFrom::End(delta) => (self.bytes.len().0, i128::from(delta)),
        };
        self.at = u64::try_from(i128::from(base) + delta)
            .map_err(|_| Error::from(ErrorKind::InvalidInput))?;
        Ok(self.at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_reads_a_range_clipped_to_what_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::write(&path, b"abcd").unwrap();
        let file = File::open(&path).unwrap();
        let vec = b"abcd".to_vec();
        let shared: Arc<[u8]> = Arc::from(&b"abcd"[..]);
        assert_eq!(vec[..].read_range(1..3).unwrap(), b"bc");
        let sources: [(&str, &dyn ReadAt); 3] = [("vec", &vec), ("arc", &shared), ("file", &file)];
        // name, start, end, bytes
        const CASES: &[(&str, u64, u64, &[u8])] = &[
            ("inside", 1, 3, b"bc"),
            ("whole", 0, 4, b"abcd"),
            ("past the end", 2, 99, b"cd"),
            ("starts past the end", 9, 12, b""),
            ("empty", 2, 2, b""),
            ("reversed", 3, 1, b""),
        ];
        for (kind, source) in sources {
            assert_eq!(source.len(), ByteLen(4), "{kind}");
            for (name, start, end, want) in CASES {
                assert_eq!(
                    source.read_range(*start..*end).unwrap(),
                    *want,
                    "{kind}: {name}"
                );
            }
        }
    }

    #[test]
    fn a_stream_over_ranged_reads_reads_and_seeks() {
        let mut stream = ReadAtStream::new(Arc::new(b"0123456789".to_vec()));
        stream.seek(SeekFrom::End(-3)).unwrap();
        let mut tail = Vec::new();
        stream.read_to_end(&mut tail).unwrap();
        assert_eq!(tail, b"789");
        assert!(stream.seek(SeekFrom::Current(-99)).is_err());
        assert_eq!(stream.seek(SeekFrom::Start(2)).unwrap(), 2);
    }
}
