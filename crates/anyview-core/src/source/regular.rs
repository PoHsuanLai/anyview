//! Opening a file only when it is a regular file: a FIFO, a device or a socket (or a link to
//! one) has no end, or never answers, so nothing that reads "the file" may be pointed at it.

use std::fs::File;
use std::io::{Error, ErrorKind};
use std::path::Path;

/// Whether `meta` describes a regular file.
pub fn is_regular(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_file()
}

/// Opens `path` and returns the file with the metadata of the open handle. A path that is not a
/// regular file once opened is refused with `ErrorKind::InvalidInput`. The path is stat-ed
/// before it is opened as well, because opening a FIFO for reading blocks until something writes
/// to it, and the check on the handle closes the window a swap would otherwise have.
pub fn open_regular(path: &Path) -> Result<(File, std::fs::Metadata), Error> {
    if !is_regular(&std::fs::metadata(path)?) {
        return Err(Error::from(ErrorKind::InvalidInput));
    }
    let file = File::open(path)?;
    let meta = file.metadata()?;
    if is_regular(&meta) {
        Ok((file, meta))
    } else {
        Err(Error::from(ErrorKind::InvalidInput))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_regular_file_opens_and_a_folder_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hi").unwrap();
        let (_, meta) = open_regular(&file).unwrap();
        assert_eq!(meta.len(), 2);
        let refused = open_regular(dir.path()).unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn a_device_reached_through_a_link_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("zero.png");
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        let refused = open_regular(&link).unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InvalidInput);
    }
}
