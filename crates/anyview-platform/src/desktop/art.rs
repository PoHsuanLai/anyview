//! The cover of the recording playing, written to a file so the desktop can show it: MPRIS names
//! its picture by a `file:` URL (`mpris:artUrl`), as every player does. One file is kept, the
//! current cover's, named by a hash of its bytes so the same cover is never written twice.

use crate::media::Artwork;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The folder inside the cache directory that holds the cover.
const FOLDER: &str = "anyview/now-playing";

/// Where the cover is kept under the person's cache directory `cache`.
pub(super) fn folder_in(cache: &Path) -> PathBuf {
    cache.join(FOLDER)
}

/// FNV-1a over `bytes`: a name for a cover, not a defence.
fn name_of(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}.png")
}

/// Write `art` into `folder` (made private to the person if it is new), remove the other covers
/// there, and return the file. `None` when it cannot be written: the desktop shows no picture,
/// and the player carries on.
pub(super) fn write(folder: &Path, art: &Artwork) -> Option<PathBuf> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(folder)
        .ok()?;
    let name = name_of(&art.png);
    let file = folder.join(&name);
    if !file.is_file() {
        // Beside the file and renamed over it, so a reader never sees half a picture.
        let partial = folder.join(format!("{name}.partial"));
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&partial)
            .ok()?;
        let written = out.write_all(&art.png).and_then(|()| out.flush());
        if written.is_err() || std::fs::rename(&partial, &file).is_err() {
            let _gone = std::fs::remove_file(&partial);
            return None;
        }
    }
    sweep(folder, &name);
    Some(file)
}

/// Remove every file in `folder` but `keep`.
fn sweep(folder: &Path, keep: &str) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name() != keep {
            let _gone = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn art(bytes: &[u8]) -> Artwork {
        Artwork {
            png: Arc::from(bytes),
        }
    }

    #[test]
    fn a_cover_is_written_once_and_the_last_one_is_all_that_is_kept() {
        let scratch = tempfile::tempdir().unwrap();
        let folder = folder_in(scratch.path());
        let first = write(&folder, &art(b"one")).unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(write(&folder, &art(b"one")).unwrap(), first, "same name");
        let second = write(&folder, &art(b"two")).unwrap();
        assert_ne!(second, first);
        assert!(!first.exists(), "the old cover is swept");
        let left: Vec<_> = std::fs::read_dir(&folder).unwrap().collect();
        assert_eq!(left.len(), 1, "one cover is kept");
    }

    #[test]
    fn a_cover_that_cannot_be_written_is_none() {
        let scratch = tempfile::tempdir().unwrap();
        let blocked = scratch.path().join("file");
        std::fs::write(&blocked, b"x").unwrap();
        assert_eq!(write(&blocked.join("in"), &art(b"one")), None);
    }
}
