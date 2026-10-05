//! The versions store: the original of every file the viewer saves in place, kept so a person can
//! go back. Layout, under the root (`<state>/anyview/versions`):
//!
//! ```text
//! <key>/<seconds>-<n>.bin    the original's bytes
//! <key>/<seconds>-<n>.json   its path, when it was kept and its size
//! ```
//!
//! `<key>` is a hash of the file's real path, so one directory holds one file's versions and
//! listing them reads that directory alone. The `.json` is written last, whole: a version without
//! one is a save that died halfway, and is invisible. `<n>` separates two versions kept in the
//! same second. This crate reads no clock; every call is handed the time.
//!
//! Where a save and its undo meet this API: [`Versions::back_up`] makes the [`BackedUp`] that
//! [`BackedUp::write_in_place`] needs; [`Versions::list`] gives a file's versions newest first;
//! [`Versions::restore`] is itself a save in place, so it keeps the current file first;
//! [`Versions::prune`] drops what is older than the keep period.

use crate::error::{StoreError, StoreOp};
use crate::io::{self, io_error};
use crate::save::{BackedUp, Pending, Written};
use anyview_core::ByteLen;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The folder under a person's state directory that holds the store (`<state>/anyview/versions`
/// is `<state>` joined with [`STORE_FOLDER`](crate::STORE_FOLDER) and this).
pub const VERSIONS_FOLDER: &str = "versions";

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

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
    key: String,
    stem: String,
}

impl VersionId {
    fn data_path(&self, root: &Path) -> PathBuf {
        root.join(&self.key).join(format!("{}.bin", self.stem))
    }

    fn sidecar_path(&self, root: &Path) -> PathBuf {
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
struct Sidecar {
    path: String,
    saved_at: SavedAt,
    size: u64,
}

/// The versions store at one root. Blocking; call it from a worker.
#[derive(Debug, Clone)]
pub struct Versions {
    root: PathBuf,
}

impl Versions {
    /// The store at `root`, created on the first version.
    pub fn new(root: impl Into<PathBuf>) -> Versions {
        Versions { root: root.into() }
    }

    /// The store under a person's state directory (`<state>/anyview/versions`).
    pub fn under_state(state: &Path) -> Versions {
        Versions::new(state.join(crate::STORE_FOLDER).join(VERSIONS_FOLDER))
    }

    /// Copies the file `pending` will replace into the store as a version made at `at`, and
    /// returns the pending bytes ready to write. Nothing of the original changes. The file must
    /// exist: there is nothing to keep of a new one.
    pub fn back_up(&self, pending: Pending, at: SavedAt) -> Result<BackedUp, StoreError> {
        let Pending { target, bytes } = pending;
        let real = fs::canonicalize(&target).map_err(|e| io_error(StoreOp::Stat, &target, &e))?;
        let mode = fs::metadata(&real)
            .map_err(|e| io_error(StoreOp::Stat, &real, &e))?
            .permissions();
        let kept = self.keep(&real, at)?;
        Ok(BackedUp::new(real, bytes, kept, mode))
    }

    /// The versions kept of `path`, newest first.
    pub fn list(&self, path: &Path) -> Result<Vec<Version>, StoreError> {
        let real = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let key = key_of(&real)?;
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

    /// Deletes every version kept more than `keep` before `now`, and the folders left empty;
    /// returns how many went. A sidecar that cannot be read is left alone for a person to look at.
    pub fn prune(&self, keep: KeepPeriod, now: SavedAt) -> Result<usize, StoreError> {
        let folders = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(io_error(StoreOp::List, &self.root, &error)),
        };
        let mut removed = 0;
        for folder in folders {
            let folder = folder
                .map_err(|e| io_error(StoreOp::List, &self.root, &e))?
                .path();
            let Some(key) = folder
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            for sidecar in io::list_json(&folder)? {
                let Ok(Some(version)) = self.read_version(&key, &sidecar) else {
                    continue;
                };
                if version.saved_at.0.saturating_add(keep.seconds()) < now.0 {
                    // The sidecar goes first: a version with data and no sidecar is not listed.
                    io::remove(&sidecar)?;
                    io::remove(&version.id.data_path(&self.root))?;
                    removed += 1;
                }
            }
            // Fails while versions remain, which is the answer wanted.
            let _ = fs::remove_dir(&folder);
        }
        Ok(removed)
    }

    /// Copies `real` into a fresh version slot and writes its sidecar.
    fn keep(&self, real: &Path, at: SavedAt) -> Result<VersionId, StoreError> {
        let key = key_of(real)?;
        let folder = self.root.join(&key);
        fs::create_dir_all(&folder).map_err(|e| io_error(StoreOp::CreateDir, &folder, &e))?;
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
        if let Err(error) = io::write_json(&id.sidecar_path(&self.root), &sidecar) {
            io::remove(&id.data_path(&self.root))?;
            return Err(error);
        }
        Ok(id)
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
    fn read_version(&self, key: &str, sidecar: &Path) -> Result<Option<Version>, StoreError> {
        let Some(stem) = sidecar.file_stem().and_then(|s| s.to_str()) else {
            return Ok(None);
        };
        let Some(content) = io::read_json::<Sidecar>(sidecar)? else {
            return Ok(None);
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
fn key_of(path: &Path) -> Result<String, StoreError> {
    let text = path.to_str().ok_or_else(|| StoreError::PathNotUtf8 {
        path: path.to_path_buf(),
    })?;
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    Ok(format!("{hash:016x}"))
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
