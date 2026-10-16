//! A file on disk as what the viewer's readers take. `anyview-core` does no I/O, so the one place a
//! path is opened for an [`Input`] is here: [`OnDisk`] makes the input of a path or a [`Source`],
//! and [`open_regular`] is the open every reader of "the file" goes through.
//!
//! Only a regular file may be read: a FIFO, a device or a socket (or a link to one) has no end,
//! or never answers.
//!
//! It is a crate of its own, and not part of `anyview-store`, because it must build everywhere the
//! headless peek does (the store reads extended attributes, which Windows has no call for).
//!
//! Every public item is reached from this root, once.
//!
//! A regular file is opened and read by offset, and any other kind of file is refused:
//!
//! ```
//! use anyview_core::{ByteLen, ReadAt};
//! use anyview_fs::OpenFile;
//!
//! let dir = tempfile::tempdir()?;
//! let path = dir.path().join("note.txt");
//! std::fs::write(&path, b"hello")?;
//!
//! let file = OpenFile::open(&path)?;
//! assert_eq!(file.len(), ByteLen(5));
//! let mut tail = [0u8; 3];
//! assert_eq!(file.read_at(2, &mut tail)?, 3);
//! assert_eq!(&tail, b"llo");
//! assert!(OpenFile::open(dir.path()).is_err(), "a folder is not a regular file");
//! # Ok::<(), std::io::Error>(())
//! ```

#![warn(missing_docs)]

use anyview_core::{ByteLen, FileName, FilePath, FileStamp, Input, ModTime, ReadAt, Source};
use std::fs::{File, Metadata};
use std::io::{Error, ErrorKind, Result};
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// Whether `meta` describes a regular file.
pub fn is_regular(meta: &Metadata) -> bool {
    meta.file_type().is_file()
}

/// Opens `path` and returns the file with the metadata of the open handle. A path that is not a
/// regular file once opened is refused with `ErrorKind::InvalidInput`. The path is stat-ed
/// before it is opened, and on Unix it is opened with `O_NONBLOCK`, because opening a FIFO for
/// reading otherwise blocks until something writes to it; the check on the handle then refuses
/// a FIFO swapped in between the two.
pub fn open_regular(path: &Path) -> Result<(File, Metadata)> {
    if !is_regular(&std::fs::metadata(path)?) {
        return Err(Error::from(ErrorKind::InvalidInput));
    }
    let file = open_nonblocking(path)?;
    let meta = file.metadata()?;
    if is_regular(&meta) {
        Ok((file, meta))
    } else {
        Err(Error::from(ErrorKind::InvalidInput))
    }
}

#[cfg(unix)]
fn open_nonblocking(path: &Path) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    // A regular file ignores the flag; a FIFO opens at once instead of waiting for a writer.
    let nonblock = i32::try_from(rustix::fs::OFlags::NONBLOCK.bits())
        .map_err(|_| Error::from(ErrorKind::Unsupported))?;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(nonblock)
        .open(path)
}

#[cfg(not(unix))]
fn open_nonblocking(path: &Path) -> Result<File> {
    File::open(path)
}

/// The most a file may be for the viewer to read it whole into memory before decoding or editing:
/// the ceiling `anyview-image` holds a decode's peak to, since a file over it cannot be decoded
/// within that peak, and a parse that allocates for a larger one would abort the whole process.
pub const WHOLE_FILE_CAP: ByteLen = ByteLen(1536 * 1024 * 1024);

/// The regular file at `path`, read whole, when it is at most `cap` long. A missing file is
/// `ErrorKind::NotFound`, anything that is not a regular file (a folder, a FIFO, a device)
/// `ErrorKind::InvalidInput`, and a file over `cap`, or one that grows past it while it is read,
/// `ErrorKind::FileTooLarge`: never a hang, and never more than `cap` bytes in memory. The read
/// is quire's `ds_helpers::read_capped`, the one the desktop's apps share.
pub fn read_whole(path: &Path, cap: ByteLen) -> Result<Vec<u8>> {
    ds_helpers::read_capped(path, ds_helpers::ReadCap::bytes(cap.0)).map_err(io_error)
}

// `ReadError` is `non_exhaustive`: what falls to the wildcard is a failure this crate cannot
// name yet, so it stays an error with its own message.
#[allow(clippy::wildcard_enum_match_arm)]
fn io_error(error: ds_helpers::ReadError) -> Error {
    match error {
        ds_helpers::ReadError::NotFound { .. } => Error::from(ErrorKind::NotFound),
        ds_helpers::ReadError::NotRegular { .. } => Error::from(ErrorKind::InvalidInput),
        ds_helpers::ReadError::TooBig { .. } => Error::from(ErrorKind::FileTooLarge),
        ds_helpers::ReadError::Io { source, .. } => source,
        other => Error::other(other),
    }
}

/// An open regular file, read by offset.
#[derive(Debug)]
pub struct OpenFile {
    file: File,
    /// The length the file had when it was opened, for when its metadata can no longer be read.
    opened: ByteLen,
}

impl OpenFile {
    /// The regular file at `path`, opened ([`open_regular`]).
    pub fn open(path: &Path) -> Result<OpenFile> {
        open_regular(path).map(|(file, meta)| OpenFile {
            file,
            opened: ByteLen(meta.len()),
        })
    }
}

impl ReadAt for OpenFile {
    /// The file's length now, or the one it had when opened if the system no longer says.
    fn len(&self) -> ByteLen {
        self.file
            .metadata()
            .map_or(self.opened, |meta| ByteLen(meta.len()))
    }

    #[cfg(unix)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        std::os::unix::fs::FileExt::read_at(&self.file, buf, offset)
    }

    #[cfg(windows)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        std::os::windows::fs::FileExt::seek_read(&self.file, buf, offset)
    }
}

/// Something that names a file on disk and can become the [`Input`] a peek reads.
pub trait OnDisk {
    /// The file as an input, opened on first read: a missing file or a FIFO is an error then, not
    /// here.
    fn on_disk(&self) -> Input;
}

/// The file at the path as it is now: its stamp is read, once, the first time it is asked for.
impl OnDisk for FilePath {
    fn on_disk(&self) -> Input {
        at_path(self, None)
    }
}

/// The file the source names, with the stamp it had when it was probed.
impl OnDisk for Source {
    fn on_disk(&self) -> Input {
        at_path(self.path(), Some(self.stamp()))
    }
}

/// A file at a path, opened on first read. The length is the one the open file has.
#[derive(Debug)]
struct PathBytes {
    path: FilePath,
    file: OnceLock<std::result::Result<OpenFile, ErrorKind>>,
}

impl PathBytes {
    fn file(&self) -> Result<&OpenFile> {
        self.file
            .get_or_init(|| OpenFile::open(self.path.as_path()).map_err(|e| e.kind()))
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

    fn stamp(&self) -> Option<FileStamp> {
        let meta = std::fs::metadata(self.path.as_path());
        Some(FileStamp {
            len: ByteLen(meta.as_ref().map_or(0, |meta| meta.len())),
            modified: meta
                .and_then(|meta| meta.modified())
                .map_or(ModTime(0), ModTime::from_system_time),
        })
    }
}

fn at_path(path: &FilePath, stamp: Option<FileStamp>) -> Input {
    let name = path.file_name().unwrap_or_else(FileName::unnamed);
    let bytes = Arc::new(PathBytes {
        path: path.clone(),
        file: OnceLock::new(),
    });
    match stamp {
        Some(stamp) => Input::new(name, stamp, bytes),
        None => Input::measured(name, bytes),
    }
    .at(path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::ModTime;
    use std::io::Read;

    #[test]
    fn a_path_reads_as_its_file_and_a_regular_file_opens_but_a_folder_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").unwrap();
        let path = FilePath::new(&file).unwrap();
        let input = path.on_disk();
        let mut text = String::new();
        input.reader().read_to_string(&mut text).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(input.bytes().len(), ByteLen(5));
        assert_eq!(input.stamp().len, ByteLen(5));
        assert_eq!(input.name().as_str(), "a.txt");
        assert_eq!(input.path(), Some(&path));
        assert_eq!(open_regular(&file).unwrap().1.len(), 5);
        assert_eq!(
            open_regular(dir.path()).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_whole_read_takes_a_regular_file_up_to_its_cap_and_names_every_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").unwrap();
        assert_eq!(read_whole(&file, ByteLen(5)).unwrap(), b"hello");
        let kind = |path: &Path, cap: u64| read_whole(path, ByteLen(cap)).unwrap_err().kind();
        assert_eq!(kind(&file, 4), ErrorKind::FileTooLarge);
        assert_eq!(kind(&dir.path().join("gone"), 5), ErrorKind::NotFound);
        assert_eq!(kind(dir.path(), 5), ErrorKind::InvalidInput);
    }

    // rustix has no mkfifo on Apple platforms; Linux is where the pipe is tried.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_named_pipe_is_refused_at_once_by_every_open() {
        let dir = tempfile::tempdir().unwrap();
        let pipe = dir.path().join("pipe");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &pipe,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        // Nothing writes to the pipe: a blocking open would never return.
        assert_eq!(
            open_regular(&pipe).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
        assert_eq!(
            read_whole(&pipe, WHOLE_FILE_CAP).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_missing_file_or_a_folder_fails_when_read_and_a_source_keeps_its_stamp() {
        let dir = tempfile::tempdir().unwrap();
        let gone = FilePath::new(dir.path().join("gone.txt")).unwrap();
        let folder = FilePath::new(dir.path()).unwrap();
        let mut one = [0u8; 1];
        assert_eq!(
            gone.on_disk()
                .bytes()
                .read_at(0, &mut one)
                .unwrap_err()
                .kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            folder
                .on_disk()
                .bytes()
                .read_at(0, &mut one)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput
        );
        let stamp = FileStamp {
            len: ByteLen(3),
            modified: ModTime(1),
        };
        let source = Source::new(gone, stamp);
        assert_eq!(source.on_disk().stamp(), stamp);
        assert_eq!(source.on_disk().bytes().len(), ByteLen(0));
    }

    #[cfg(unix)]
    #[test]
    fn a_device_reached_through_a_link_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("zero.png");
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        assert_eq!(
            open_regular(&link).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
    }
}
