//! Writing a file whole: to a temporary file beside the destination, then renamed into place.

use crate::error::ExportError;
use std::io::Write;
use std::path::{Path, PathBuf};

/// `bytes` as the new file `to`. The bytes go to a hidden file in the same folder first and are
/// renamed to `to` only when they are all on disk, so `to` never holds part of them. A `to` that
/// exists is left as it is, whoever made it.
pub(crate) fn write_new(bytes: &[u8], to: &Path) -> Result<(), ExportError> {
    let write_error = |error: std::io::Error| ExportError::Write {
        path: to.to_path_buf(),
        kind: error.kind(),
    };
    let partial = partial_beside(to);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(error) = written {
        let _gone = std::fs::remove_file(&partial);
        return Err(write_error(error));
    }
    if to.exists() {
        let _gone = std::fs::remove_file(&partial);
        return Err(ExportError::Exists {
            path: to.to_path_buf(),
        });
    }
    std::fs::rename(&partial, to).map_err(|error| {
        let _gone = std::fs::remove_file(&partial);
        write_error(error)
    })
}

/// The hidden name the bytes are written under before `to` has them.
fn partial_beside(to: &Path) -> PathBuf {
    let name = to
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    to.with_file_name(format!(".{name}.{}.part", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn a_written_file_is_whole_and_leaves_no_temporary_one() {
        let dir = tempfile::tempdir().unwrap();
        let to = dir.path().join("out.txt");
        write_new(b"all of it", &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"all of it");
        assert_eq!(names(dir.path()), ["out.txt"]);
    }

    #[test]
    fn a_file_that_is_there_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let to = dir.path().join("out.txt");
        std::fs::write(&to, "mine").unwrap();
        let error = write_new(b"theirs", &to).unwrap_err();
        assert!(matches!(error, ExportError::Exists { .. }), "{error}");
        assert_eq!(std::fs::read(&to).unwrap(), b"mine");
        assert_eq!(names(dir.path()), ["out.txt"], "the temporary file is gone");
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_error_and_makes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let to = dir.path().join("missing").join("out.txt");
        assert!(matches!(
            write_new(b"x", &to),
            Err(ExportError::Write { .. })
        ));
        assert!(names(dir.path()).is_empty());
    }
}
