//! `Input`: what a peek reads. A path is the built-in one; a host injects any other bytes.

use super::{ByteLen, FileName, FilePath, FileStamp, ModTime, ReadAt, ReadAtStream, Source};
use std::fs::File;
use std::io::{ErrorKind, Result};
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// A file to look at, wherever its bytes are: its name (which sniffing reads the extension from),
/// the stamp of this version of it (which keys caches), the bytes, and the path they are at when
/// they are in a file of their own. Cheap to clone; the bytes are shared.
///
/// Back ends that can only read a path (a PDF rasteriser, a folder listing) use [`Input::path`]
/// and refuse cleanly when there is none, unless the caller spools the bytes to a file first.
/// [`Source`] stays the serialisable identity of a file at a path.
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
            Stamped::OfPath(path, stamp) => *stamp.get_or_init(|| stamp_of(path)),
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

/// The stamp of an input: given, or read from the file the first time it is asked for, so
/// making an input of a path costs nothing and the stat happens on the worker that peeks.
#[derive(Debug, Clone)]
enum Stamped {
    Known(FileStamp),
    OfPath(FilePath, OnceLock<FileStamp>),
}

/// A file at a path, opened on first read: a missing file or a FIFO is an error then, not at
/// construction. The length is the one the open file has.
#[derive(Debug)]
struct PathBytes {
    path: FilePath,
    file: OnceLock<std::result::Result<File, ErrorKind>>,
}

impl PathBytes {
    fn file(&self) -> Result<&File> {
        self.file
            .get_or_init(|| {
                super::open_regular(self.path.as_path())
                    .map(|(file, _)| file)
                    .map_err(|e| e.kind())
            })
            .as_ref()
            .map_err(|kind| (*kind).into())
    }
}

impl ReadAt for PathBytes {
    fn len(&self) -> ByteLen {
        self.file().map_or(ByteLen(0), |file| file.len())
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.file()?.read_at(offset, buf)
    }
}

fn stamp_of(path: &FilePath) -> FileStamp {
    let meta = std::fs::metadata(path.as_path());
    FileStamp {
        len: ByteLen(meta.as_ref().map_or(0, |meta| meta.len())),
        modified: meta
            .and_then(|meta| meta.modified())
            .map_or(ModTime(0), ModTime::from_system_time),
    }
}

fn at_path(path: &FilePath, stamp: Stamped) -> Input {
    let name = path.file_name().unwrap_or_else(FileName::unnamed);
    let bytes = Arc::new(PathBytes {
        path: path.clone(),
        file: OnceLock::new(),
    });
    Input {
        name,
        stamp,
        bytes,
        path: Some(path.clone()),
    }
}

/// The file at `path` as it is now.
impl From<&FilePath> for Input {
    fn from(path: &FilePath) -> Self {
        at_path(path, Stamped::OfPath(path.clone(), OnceLock::new()))
    }
}

/// The file `source` names, with the stamp it had when it was probed.
impl From<&Source> for Input {
    fn from(source: &Source) -> Self {
        at_path(source.path(), Stamped::Known(source.stamp()))
    }
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
    use std::io::Read;

    #[test]
    fn a_path_reads_as_its_file_and_bytes_read_as_themselves() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").unwrap();
        let path = FilePath::new(&file).unwrap();
        let held = Input::from((FileName::new("a.txt").unwrap(), b"hello".to_vec()));
        for (name, input) in [("path", Input::from(&path)), ("held", held)] {
            let mut text = String::new();
            input.reader().read_to_string(&mut text).unwrap();
            assert_eq!(text, "hello", "{name}");
            assert_eq!(input.bytes().len(), ByteLen(5), "{name}");
            assert_eq!(input.stamp().len, ByteLen(5), "{name}");
            assert_eq!(input.name().as_str(), "a.txt", "{name}");
        }
        assert!(Input::from(&path).path().is_some());
    }

    #[test]
    fn a_missing_file_or_a_folder_fails_when_read_and_a_source_keeps_its_stamp() {
        let dir = tempfile::tempdir().unwrap();
        let gone = FilePath::new(dir.path().join("gone.txt")).unwrap();
        let folder = FilePath::new(dir.path()).unwrap();
        let mut one = [0u8; 1];
        let missing = Input::from(&gone);
        assert_eq!(
            missing.bytes().read_at(0, &mut one).unwrap_err().kind(),
            ErrorKind::NotFound
        );
        let refused = Input::from(&folder);
        assert_eq!(
            refused.bytes().read_at(0, &mut one).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
        let stamp = FileStamp {
            len: ByteLen(3),
            modified: ModTime(1),
        };
        let source = Source::new(gone, stamp);
        assert_eq!(Input::from(&source).stamp(), stamp);
        assert_eq!(Input::from(&source).bytes().len(), ByteLen(0));
    }
}
