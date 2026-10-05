//! Keeping the versions store small: what is too old, what a crash left half made, and the
//! oldest versions past the size cap.

use crate::error::{StoreError, StoreOp};
use crate::io::{self, io_error};
use crate::versions::{KeepPeriod, SECONDS_PER_DAY, SavedAt, Version, Versions};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// How long a half-made file stays before it is called a leftover: a save in progress is
/// writing one now.
const LEFTOVER_AGE: u64 = SECONDS_PER_DAY;

/// How far past the newest kept version the clock may be before nothing is deleted by age: a
/// clock set years ahead (a dead battery, a resumed virtual machine) would otherwise erase every
/// version at once. The versions wait until a save puts a newer one beside them.
const CLOCK_SLACK: u64 = 365 * SECONDS_PER_DAY;

/// One folder of the store as the prune finds it.
struct Folder {
    path: PathBuf,
    versions: Vec<Version>,
    /// Files no sidecar describes and a crash can leave: a `.bin` with no `.json` at all, and a
    /// `.json.tmp`. Only the old ones are listed.
    leftovers: Vec<PathBuf>,
}

impl Versions {
    /// Deletes what the store no longer needs and returns how many versions went:
    /// - every version kept more than `keep` before `now`, unless the clock is more than a year
    ///   past the newest version, when it is not to be believed and nothing goes by age;
    /// - the files a crash left half made, and the folders left empty;
    /// - then the oldest versions, while all together take more than the store's cap, never a
    ///   file's newest.
    ///
    /// A sidecar that cannot be read is left alone for a person to look at.
    pub fn prune(&self, keep: KeepPeriod, now: SavedAt) -> Result<usize, StoreError> {
        let mut folders = self.survey(now)?;
        let newest = folders
            .iter()
            .flat_map(|folder| &folder.versions)
            .map(|version| version.saved_at.0)
            .max();
        if newest.is_some_and(|newest| now.0 > newest.saturating_add(CLOCK_SLACK)) {
            return Ok(0);
        }
        let mut removed = 0;
        for folder in &mut folders {
            for leftover in &folder.leftovers {
                io::remove(leftover)?;
            }
            let (old, young): (Vec<_>, Vec<_>) = std::mem::take(&mut folder.versions)
                .into_iter()
                .partition(|version| version.saved_at.0.saturating_add(keep.seconds()) < now.0);
            for version in &old {
                self.delete(version)?;
            }
            removed += old.len();
            folder.versions = young;
        }
        removed += self.trim_to_cap(&mut folders)?;
        for folder in &folders {
            // Fails while versions remain, which is the answer wanted.
            let _kept = fs::remove_dir(&folder.path);
        }
        Ok(removed)
    }

    /// Deletes the oldest versions until the rest fit the cap; every file keeps its newest.
    fn trim_to_cap(&self, folders: &mut [Folder]) -> Result<usize, StoreError> {
        let mut total: u64 = folders
            .iter()
            .flat_map(|folder| &folder.versions)
            .map(|version| version.size.0)
            .sum();
        if total <= self.cap.0 {
            return Ok(0);
        }
        let mut newest: HashMap<&Path, &Version> = HashMap::new();
        for version in folders.iter().flat_map(|folder| &folder.versions) {
            let slot = newest.entry(version.path.as_path()).or_insert(version);
            if (version.saved_at, &version.id.stem) > (slot.saved_at, &slot.id.stem) {
                *slot = version;
            }
        }
        let mut candidates: Vec<&Version> = folders
            .iter()
            .flat_map(|folder| &folder.versions)
            .filter(|version| {
                !newest
                    .get(version.path.as_path())
                    .is_some_and(|latest| std::ptr::eq(*latest, *version))
            })
            .collect();
        candidates.sort_by(|a, b| (a.saved_at, &a.id.stem).cmp(&(b.saved_at, &b.id.stem)));
        let mut removed = 0;
        for version in candidates {
            if total <= self.cap.0 {
                break;
            }
            self.delete(version)?;
            total = total.saturating_sub(version.size.0);
            removed += 1;
        }
        Ok(removed)
    }

    /// The sidecar goes first: a version with data and no sidecar is not listed.
    fn delete(&self, version: &Version) -> Result<(), StoreError> {
        io::remove(&version.id.sidecar_path(&self.root))?;
        io::remove(&version.id.data_path(&self.root))
    }

    /// Every folder of the store: its readable versions and its old leftovers.
    fn survey(&self, now: SavedAt) -> Result<Vec<Folder>, StoreError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(StoreOp::List, &self.root, &error)),
        };
        let mut folders = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|e| io_error(StoreOp::List, &self.root, &e))?
                .path();
            let Some(key) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if !path.is_dir() {
                continue;
            }
            let mut versions = Vec::new();
            for sidecar in io::list_json(&path)? {
                if let Some(version) = self.read_version(&key, &sidecar)? {
                    versions.push(version);
                }
            }
            let leftovers = leftovers_in(&path, now)?;
            folders.push(Folder {
                path,
                versions,
                leftovers,
            });
        }
        Ok(folders)
    }
}

/// The half-made files in `folder` that are at least a day old.
fn leftovers_in(folder: &Path, now: SavedAt) -> Result<Vec<PathBuf>, StoreError> {
    let entries = fs::read_dir(folder).map_err(|e| io_error(StoreOp::List, folder, &e))?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| io_error(StoreOp::List, folder, &e))?
            .path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let half_made = name.ends_with(".json.tmp")
            || (name.ends_with(".bin") && !path.with_extension("json").exists());
        if half_made && is_old(&path, now) {
            found.push(path);
        }
    }
    Ok(found)
}

fn is_old(path: &Path, now: SavedAt) -> bool {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs());
    modified.is_some_and(|modified| modified.saturating_add(LEFTOVER_AGE) < now.0)
}
