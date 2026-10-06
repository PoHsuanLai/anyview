//! Writing a file whole: to a temporary file beside the destination, then given its name.

use crate::error::ExportError;
use anyview_store::{StoreError, is_taken, link_new, partial_beside, sweep_leftovers};
use std::io::Write;
use std::path::Path;

/// `bytes` as the new file `to`. The bytes go to a hidden file in the same folder first and are
/// given the name `to` only when they are all on disk, so `to` never holds part of them. A `to`
/// that exists is left as it is, whoever made it, and so is a hidden file this call did not make.
/// The hidden files of an export that died are removed first.
pub fn write_new(bytes: &[u8], to: &Path) -> Result<(), ExportError> {
    let write_error = |error: std::io::Error| ExportError::Write {
        path: to.to_path_buf(),
        kind: error.kind(),
    };
    sweep_leftovers(to.parent().unwrap_or_else(|| Path::new(".")));
    let partial = partial_beside(to);
    // Only a file this call made is this call's to remove.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(write_error)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let placed = match written {
        Ok(()) => link_new(&partial, to).map_err(|error| match error {
            error if is_taken(&error) => ExportError::Exists {
                path: to.to_path_buf(),
            },
            StoreError::Io { kind, .. } => ExportError::Write {
                path: to.to_path_buf(),
                kind,
            },
            StoreError::Corrupt { .. }
            | StoreError::NoSuchVersion { .. }
            | StoreError::PathNotUtf8 { .. } => ExportError::Write {
                path: to.to_path_buf(),
                kind: std::io::ErrorKind::Other,
            },
        }),
        Err(error) => Err(write_error(error)),
    };
    let _gone = std::fs::remove_file(&partial);
    placed
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
    fn a_dangling_symlink_is_a_taken_name_and_makes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let to = dir.path().join("out.txt");
        let elsewhere = dir.path().join("elsewhere");
        std::os::unix::fs::symlink(&elsewhere, &to).unwrap();
        let error = write_new(b"x", &to).unwrap_err();
        assert!(matches!(error, ExportError::Exists { .. }), "{error}");
        assert!(!elsewhere.exists(), "the link's target was created");
    }

    #[test]
    fn a_hidden_file_of_a_dead_export_is_swept_and_a_live_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        // 4194304 is above the kernel's largest pid, so no process has it.
        let dead = dir.path().join(".out.txt.4194304-1.part");
        let live = dir
            .path()
            .join(format!(".other.txt.{}-99.part", std::process::id()));
        std::fs::write(&dead, "x").unwrap();
        std::fs::write(&live, "y").unwrap();
        write_new(b"all", &dir.path().join("out.txt")).unwrap();
        assert!(!dead.exists());
        assert!(live.exists());
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
