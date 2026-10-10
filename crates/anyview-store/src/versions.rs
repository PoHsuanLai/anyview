//! The versions store: the original of every file the viewer saves in place, kept so a person can
//! go back. Layout, under the root (`<state>/anyview/versions`):
//!
//! ```text
//! <key>/<seconds>-<n>.bin    the original's bytes
//! <key>/<seconds>-<n>.json   its path, when it was kept and its size
//! ```
//!
//! A kept version is a private copy of a person's file: the folders are `0700` and the files
//! `0600`, whatever the original's mode.
//!
//! `<key>` is a hash of the file's real path, so one directory holds one file's versions and
//! listing them reads that directory alone. The `.json` is written last, whole: a version without
//! one is a save that died halfway, and is invisible. `<n>` separates two versions kept in the
//! same second. This crate reads no clock; every call is handed the time.
//!
//! Where a save and its undo meet this API: [`Versions::back_up`] makes the [`BackedUp`] that
//! [`BackedUp::write_in_place`] needs; [`Versions::list`] gives a file's versions newest first;
//! [`Versions::restore`] is itself a save in place, so it keeps the current file first;
//! [`Versions::prune`] drops what is older than the keep period, what a crash left half made, and
//! the oldest versions past the store's size cap; [`Versions::rekey`] follows a renamed file.

use crate::attrs::is_locked;
use crate::error::{StoreError, StoreOp};
use crate::io::{self, io_error};
use crate::original::Original;
use crate::save::{BackedUp, Pending, Written};
use anyview_core::ByteLen;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The folder under a person's state directory that holds the store (`<state>/anyview/versions`
/// is `<state>` joined with [`STORE_FOLDER`](crate::STORE_FOLDER) and this).
pub const VERSIONS_FOLDER: &str = "versions";

pub(crate) const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// The most the kept versions take together before [`Versions::prune`] drops the oldest.
pub const DEFAULT_CAP: ByteLen = ByteLen(2 * 1024 * 1024 * 1024);

/// Seconds since the Unix epoch, when a version was kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SavedAt(pub u64);

/// How long a kept version stays before [`Versions::prune`] may delete it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeepPeriod {
    days: u32,
}

/// What the viewer keeps versions for unless the person says otherwise.
pub const DEFAULT_KEEP: KeepPeriod = KeepPeriod::days(30);

impl KeepPeriod {
    /// A period of `days` whole days; zero keeps nothing past the second it was kept in.
    pub const fn days(days: u32) -> KeepPeriod {
        KeepPeriod { days }
    }

    /// The period in seconds.
    pub fn seconds(self) -> u64 {
        u64::from(self.days) * SECONDS_PER_DAY
    }
}

/// Names one kept version. Get one from [`Versions::list`] or a [`Written`]; its text is for
/// logs, not for parsing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionId {
    pub(crate) key: String,
    pub(crate) stem: String,
}

impl VersionId {
    pub(crate) fn data_path(&self, root: &Path) -> PathBuf {
        root.join(&self.key).join(format!("{}.bin", self.stem))
    }

    pub(crate) fn sidecar_path(&self, root: &Path) -> PathBuf {
        root.join(&self.key).join(format!("{}.json", self.stem))
    }
}

impl std::fmt::Display for VersionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.key, self.stem)
    }
}

/// One kept original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// What to pass to [`Versions::restore`].
    pub id: VersionId,
    /// The file it was kept from, as its real path.
    pub path: PathBuf,
    /// When it was kept.
    pub saved_at: SavedAt,
    /// Its size in bytes.
    pub size: ByteLen,
}

/// The sidecar file's content.
#[derive(Serialize, Deserialize)]
pub(crate) struct Sidecar {
    pub(crate) path: String,
    pub(crate) saved_at: SavedAt,
    pub(crate) size: u64,
}

/// Whether a version was made for the save that asked, or was already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// Made just now: a save that fails before it touches the file has no use for it.
    Fresh,
    /// The file's latest version already held these bytes, so none was made.
    Reused,
}

/// A version a save relies on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Kept {
    pub(crate) id: VersionId,
    pub(crate) origin: Origin,
}

/// The versions store at one root. Blocking; call it from a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions {
    pub(crate) root: PathBuf,
    pub(crate) cap: ByteLen,
}

impl Versions {
    /// The store at `root`, created on the first version.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Versions {
        Versions {
            root: root.into(),
            cap: DEFAULT_CAP,
        }
    }

    /// The same store with room for `cap` bytes of versions instead of [`DEFAULT_CAP`].
    #[must_use]
    pub fn with_cap(self, cap: ByteLen) -> Versions {
        Versions { cap, ..self }
    }

    /// The store under a person's state directory (`<state>/anyview/versions`).
    #[must_use]
    pub fn under_state(state: &Path) -> Versions {
        Versions::new(state.join(crate::STORE_FOLDER).join(VERSIONS_FOLDER))
    }

    /// Keeps the file `pending` will replace as a version made at `at`, and returns the pending
    /// bytes ready to write. Nothing of the original changes. The file must exist: there is
    /// nothing to keep of a new one. A file that is read-only is refused (`Permissions`,
    /// `PermissionDenied`): a person unlocks it first, as in any editor. When the file's latest
    /// version already holds the same bytes, that one is the version and no copy is made.
    pub fn back_up(&self, pending: Pending, at: SavedAt) -> Result<BackedUp, StoreError> {
        let target = &pending.target;
        let real = fs::canonicalize(target).map_err(|e| io_error(StoreOp::Stat, target, &e))?;
        let meta = fs::metadata(&real).map_err(|e| io_error(StoreOp::Stat, &real, &e))?;
        if is_locked(&real, &meta) {
            let denied = std::io::Error::from(ErrorKind::PermissionDenied);
            return Err(io_error(StoreOp::Permissions, &real, &denied));
        }
        let kept = self.keep_as_is(&real, at)?;
        let pending = Pending::new(real, pending.bytes);
        Ok(BackedUp::new(
            pending,
            kept,
            Original::of(&meta),
            self.clone(),
            at,
        ))
    }

    /// The versions kept of `path`, newest first. A sidecar that is not readable is left out, so
    /// one damaged version never hides the others.
    pub fn list(&self, path: &Path) -> Result<Vec<Version>, StoreError> {
        let real = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.versions_of(&real)
    }

    /// The versions kept of the real path `real`, newest first.
    pub(crate) fn versions_of(&self, real: &Path) -> Result<Vec<Version>, StoreError> {
        let key = key_of(real)?;
        let mut versions = Vec::new();
        for sidecar in io::list_json(&self.root.join(&key))? {
            let Some(version) = self.read_version(&key, &sidecar)? else {
                continue;
            };
            if version.path == real {
                versions.push(version);
            }
        }
        versions.sort_by(|a, b| (b.saved_at, &b.id.stem).cmp(&(a.saved_at, &a.id.stem)));
        Ok(versions)
    }

    /// Puts a kept version back as the file's content. That is a save in place like any other:
    /// the file as it is now is kept first (at `at`), so a restore can itself be undone.
    pub fn restore(&self, id: &VersionId, at: SavedAt) -> Result<Written, StoreError> {
        let sidecar: Sidecar = io::read_json(&id.sidecar_path(&self.root))?
            .ok_or_else(|| StoreError::NoSuchVersion { id: id.to_string() })?;
        let data = id.data_path(&self.root);
        let bytes = fs::read(&data).map_err(|e| io_error(StoreOp::Read, &data, &e))?;
        self.back_up(Pending::new(sidecar.path, bytes), at)?
            .write_in_place()
    }

    /// The latest version of `real` when it holds these very bytes, else a fresh one.
    fn keep_as_is(&self, real: &Path, at: SavedAt) -> Result<Kept, StoreError> {
        if let Some(latest) = self.versions_of(real)?.into_iter().next()
            && same_bytes(&latest.id.data_path(&self.root), real)
        {
            return Ok(Kept {
                id: latest.id,
                origin: Origin::Reused,
            });
        }
        self.keep(real, at).map(|id| Kept {
            id,
            origin: Origin::Fresh,
        })
    }

    /// [`keep_as_is`](Self::keep_as_is) for the file as it is now, when someone changed it
    /// after the backup.
    pub(crate) fn keep_current(&self, real: &Path, at: SavedAt) -> Result<Kept, StoreError> {
        self.keep_as_is(real, at)
    }

    /// Deletes a version this store made for a save that did not happen.
    pub(crate) fn forget(&self, id: &VersionId) {
        let _gone = io::remove(&id.sidecar_path(&self.root));
        let _gone = io::remove(&id.data_path(&self.root));
        // Fails while other versions remain, which is the answer wanted.
        let _kept = fs::remove_dir(self.root.join(&id.key));
    }

    /// Copies `real` into a fresh version slot and writes its sidecar.
    fn keep(&self, real: &Path, at: SavedAt) -> Result<VersionId, StoreError> {
        let key = key_of(real)?;
        let folder = self.root.join(&key);
        io::create_private_dir(&self.root)?;
        io::create_private_dir(&folder)?;
        let (id, size) = self.copy_into_free_slot(real, &key, at)?;
        let sidecar = Sidecar {
            path: real
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| StoreError::PathNotUtf8 {
                    path: real.to_path_buf(),
                })?,
            saved_at: at,
            size,
        };
        if let Err(error) = io::write_private_json(&id.sidecar_path(&self.root), &sidecar) {
            io::remove(&id.data_path(&self.root))?;
            return Err(error);
        }
        Ok(id)
    }

    /// A free slot under `key` holding `data`'s bytes: a hard link, which is instant and takes no
    /// room, or a copy where the file system has none.
    pub(crate) fn claim_slot(
        &self,
        key: &str,
        data: &Path,
        at: SavedAt,
    ) -> Result<VersionId, StoreError> {
        for n in 0_u32..10_000 {
            let id = VersionId {
                key: key.to_owned(),
                stem: format!("{:012}-{n:04}", at.0),
            };
            match fs::hard_link(data, id.data_path(&self.root)) {
                Ok(()) => return Ok(id),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(_) => return self.copy_into_free_slot(data, key, at).map(|(id, _)| id),
            }
        }
        Err(io_error(
            StoreOp::Write,
            &self.root.join(key),
            &ErrorKind::AlreadyExists.into(),
        ))
    }

    /// Claims the first unused `<seconds>-<n>.bin` with `create_new`, so two versions made in the
    /// same second never share a slot, and copies the original into it.
    fn copy_into_free_slot(
        &self,
        real: &Path,
        key: &str,
        at: SavedAt,
    ) -> Result<(VersionId, u64), StoreError> {
        let mut source = fs::File::open(real).map_err(|e| io_error(StoreOp::Read, real, &e))?;
        for n in 0_u32.. {
            let id = VersionId {
                key: key.to_owned(),
                stem: format!("{:012}-{n:04}", at.0),
            };
            let data = id.data_path(&self.root);
            let mut slot = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&data)
            {
                Ok(slot) => slot,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(StoreOp::Write, &data, &error)),
            };
            let copied = std::io::copy(&mut source, &mut slot)
                .and_then(|size| slot.sync_all().map(|()| size));
            return match copied {
                Ok(size) => Ok((id, size)),
                Err(error) => {
                    let _ = fs::remove_file(&data);
                    Err(io_error(StoreOp::Write, &data, &error))
                }
            };
        }
        unreachable!("the loop over every slot number only leaves by returning")
    }

    /// The version a sidecar describes; `None` when the sidecar is gone.
    pub(crate) fn read_version(
        &self,
        key: &str,
        sidecar: &Path,
    ) -> Result<Option<Version>, StoreError> {
        let Some(stem) = sidecar.file_stem().and_then(|s| s.to_str()) else {
            return Ok(None);
        };
        let content = match io::read_json::<Sidecar>(sidecar) {
            Ok(Some(content)) => content,
            // A sidecar that is gone, or damaged, describes no version; a person may look at it.
            Ok(None) | Err(StoreError::Corrupt { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(Version {
            id: VersionId {
                key: key.to_owned(),
                stem: stem.to_owned(),
            },
            path: PathBuf::from(content.path),
            saved_at: content.saved_at,
            size: ByteLen(content.size),
        }))
    }
}

/// The folder name for `path`: its FNV-1a hash in hex. Written out rather than taken from the
/// standard library's hasher, whose output may change between Rust releases and would orphan
/// every kept version. Two paths that collide share a folder and are told apart by the sidecar.
pub(crate) fn key_of(path: &Path) -> Result<String, StoreError> {
    let text = path.to_str().ok_or_else(|| StoreError::PathNotUtf8 {
        path: path.to_path_buf(),
    })?;
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    Ok(format!("{hash:016x}"))
}

/// Whether the two files hold the same bytes. A file that cannot be read is not the same.
fn same_bytes(a: &Path, b: &Path) -> bool {
    let (Ok(mut a), Ok(mut b)) = (fs::File::open(a), fs::File::open(b)) else {
        return false;
    };
    if a.metadata().ok().map(|m| m.len()) != b.metadata().ok().map(|m| m.len()) {
        return false;
    }
    let (mut left, mut right) = (vec![0_u8; 64 * 1024], vec![0_u8; 64 * 1024]);
    loop {
        let (Ok(read), Ok(other)) = (read_full(&mut a, &mut left), read_full(&mut b, &mut right))
        else {
            return false;
        };
        if read != other || left[..read] != right[..other] {
            return false;
        }
        if read == 0 {
            return true;
        }
    }
}

/// Fills `buffer` as far as the file goes.
fn read_full(file: &mut fs::File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEEP_CASES: &[(&str, KeepPeriod, u64)] = &[
        ("zero keeps nothing", KeepPeriod::days(0), 0),
        ("one day", KeepPeriod::days(1), 86_400),
        ("the default is thirty days", DEFAULT_KEEP, 2_592_000),
    ];

    #[test]
    fn keep_period_counts_whole_days_in_seconds() {
        for (name, period, seconds) in KEEP_CASES {
            assert_eq!(period.seconds(), *seconds, "{name}");
        }
    }

    #[test]
    fn a_folder_key_is_the_stable_fnv_hash_of_the_path() {
        // FNV-1a 64 of the empty string is its offset basis; the other is of "a".
        assert_eq!(key_of(Path::new("")).unwrap(), "cbf29ce484222325");
        assert_eq!(key_of(Path::new("a")).unwrap(), "af63dc4c8601ec8c");
    }
}
