//! `Input`: what a peek reads. A path is the built-in one; a host injects any other bytes.

use super::{FileName, FilePath, FileStamp, ModTime, ReadAt, ReadAtStream};
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// A file to look at, wherever its bytes are: its name (which sniffing reads the extension from),
/// the stamp of this version of it (which keys caches), the bytes, and the path they are at when
/// they are in a file of their own. Cheap to clone; the bytes are shared.
///
/// Back ends that can only read a path (a PDF rasteriser, a folder listing) use [`Input::path`]
/// and refuse cleanly when there is none, unless the caller spools the bytes to a file first.
/// [`Source`](super::Source) stays the serialisable identity of a file at a path. This crate does
/// no I/O, so an input of a file on disk is made by `anyview-fs` (`OnDisk::on_disk`).
#[derive(Debug, Clone)]
pub struct Input {
    name: FileName,
    stamp: Stamped,
    bytes: Arc<dyn ReadAt>,
    path: Option<FilePath>,
}

impl Input {
    /// `bytes` called `name`, as they were at `stamp`, in no file of their own.
    pub fn new(name: FileName, stamp: FileStamp, bytes: Arc<dyn ReadAt>) -> Self {
        Input {
            name,
            stamp: Stamped::Known(stamp),
            bytes,
            path: None,
        }
    }

    /// `bytes` called `name`, whose stamp is read from the bytes ([`ReadAt::stamp`]) the first time
    /// it is asked for, and is a length with no date when they have none.
    pub fn measured(name: FileName, bytes: Arc<dyn ReadAt>) -> Self {
        Input {
            name,
            stamp: Stamped::Measured(OnceLock::new()),
            bytes,
            path: None,
        }
    }

    /// This input, saying the bytes are the file at `path`.
    pub fn at(self, path: FilePath) -> Self {
        Input {
            path: Some(path),
            ..self
        }
    }

    /// The name sniffing reads.
    pub fn name(&self) -> &FileName {
        &self.name
    }

    /// The size and modification time of this version of the bytes.
    pub fn stamp(&self) -> FileStamp {
        match &self.stamp {
            Stamped::Known(stamp) => *stamp,
            Stamped::Measured(stamp) => *stamp.get_or_init(|| {
                self.bytes.stamp().unwrap_or_else(|| FileStamp {
                    len: self.bytes.len(),
                    modified: ModTime(0),
                })
            }),
        }
    }

    /// The bytes.
    pub fn bytes(&self) -> &Arc<dyn ReadAt> {
        &self.bytes
    }

    /// The path the bytes are at, when they are in a file of their own.
    pub fn path(&self) -> Option<&FilePath> {
        self.path.as_ref()
    }

    /// A stream over the bytes, at their start.
    pub fn reader(&self) -> ReadAtStream {
        ReadAtStream::new(Arc::clone(&self.bytes))
    }

    /// What to call this input in a message: its path, else its name.
    pub fn label(&self) -> std::path::PathBuf {
        match &self.path {
            Some(path) => path.as_path().to_path_buf(),
            None => Path::new(self.name.as_str()).to_path_buf(),
        }
    }
}

/// The stamp of an input: given, or read from the bytes the first time it is asked for, so
/// making an input of a path costs nothing and the stat happens on the worker that peeks.
#[derive(Debug, Clone)]
enum Stamped {
    Known(FileStamp),
    Measured(OnceLock<FileStamp>),
}

/// `bytes` called `name`, in memory, with no clock.
impl From<(FileName, Vec<u8>)> for Input {
    fn from((name, bytes): (FileName, Vec<u8>)) -> Self {
        let stamp = FileStamp {
            len: ReadAt::len(&bytes),
            modified: ModTime(0),
        };
        Input::new(name, stamp, Arc::new(bytes))
    }
}

/// Two inputs are the same file version when they have one name and stamp and either one path or
/// one set of shared bytes.
impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.stamp() == other.stamp()
            && (Arc::ptr_eq(&self.bytes, &other.bytes)
                || (self.path.is_some() && self.path == other.path))
    }
}

impl Eq for Input {}

impl From<&Input> for Input {
    fn from(input: &Input) -> Self {
        input.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ByteLen;
    use std::io::Read;

    /// Bytes that know their own stamp, as a file does.
    #[derive(Debug)]
    struct Dated(Vec<u8>);

    impl ReadAt for Dated {
        fn len(&self) -> ByteLen {
            ReadAt::len(&self.0)
        }

        fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.read_at(offset, buf)
        }

        fn stamp(&self) -> Option<FileStamp> {
            Some(FileStamp {
                len: ByteLen(99),
                modified: ModTime(7),
            })
        }
    }

    #[test]
    fn held_and_measured_bytes_read_as_themselves_and_stamp_as_they_say() {
        let name = || FileName::new("a.txt").unwrap();
        let held = Input::from((name(), b"hello".to_vec()));
        let measured = Input::measured(name(), Arc::new(Dated(b"hello".to_vec())));
        let undated = Input::measured(name(), Arc::new(b"hello".to_vec()));
        // name, input, the stamp it gives
        let cases = [
            ("held", held, (5, 0)),
            ("measured by the bytes", measured, (99, 7)),
            ("measured, bytes with no clock", undated, (5, 0)),
        ];
        for (case, input, (len, modified)) in cases {
            let mut text = String::new();
            input.reader().read_to_string(&mut text).unwrap();
            assert_eq!(text, "hello", "{case}");
            assert_eq!(input.stamp().len, ByteLen(len), "{case}");
            assert_eq!(input.stamp().modified, ModTime(modified), "{case}");
            assert_eq!(input.name().as_str(), "a.txt", "{case}");
            assert!(input.path().is_none(), "{case}");
        }
    }
}
