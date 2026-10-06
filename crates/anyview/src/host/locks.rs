//! Which files refuse a save in place, from the permissions the store checks before it writes: the
//! window hides the edits of such a file instead of offering what the host would decline.

use anyview_core::FilePath;
use anyview_ui::{FileAccess, FileLocks};

/// `anyview_store::is_read_only` as the views' source of file permissions.
#[derive(Debug, Clone, Copy, Default)]
pub struct StoreLocks;

impl FileLocks for StoreLocks {
    fn access(&self, path: &FilePath) -> FileAccess {
        if anyview_store::is_read_only(path.as_path()) {
            FileAccess::ReadOnly
        } else {
            FileAccess::Writable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn access_of(mode: Option<u32>) -> FileAccess {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.png");
        if let Some(mode) = mode {
            std::fs::write(&path, b"x").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        StoreLocks.access(&FilePath::new(&path).unwrap())
    }

    #[test]
    fn a_file_without_a_write_bit_is_read_only_and_the_others_are_not() {
        // name, the file's mode (none: no file), what the window may do
        const CASES: &[(&str, Option<u32>, FileAccess)] = &[
            ("no write bit", Some(0o444), FileAccess::ReadOnly),
            ("writable", Some(0o644), FileAccess::Writable),
            ("no such file", None, FileAccess::Writable),
        ];
        for (name, mode, want) in CASES {
            assert_eq!(access_of(*mode), *want, "{name}");
        }
    }
}
