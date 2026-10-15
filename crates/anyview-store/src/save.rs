//! The save pipeline as three types: [`Pending`] bytes, [`BackedUp`] bytes whose original is in
//! the versions store, and [`Written`]. [`BackedUp::write_in_place`] is the one function in the
//! program that writes over a person's file, and a `BackedUp` can only be made by
//! [`Versions::back_up`](crate::Versions::back_up), so an in-place save without the original
//! kept does not compile.

use crate::attrs::copy_attributes;
use crate::error::{StoreError, StoreOp};
use crate::guard::SaveTurn;
use crate::io::io_error;
use crate::original::Original;
use crate::place::save_temp_beside;
use crate::sweep::sweep_leftovers;
use crate::sys;
use crate::versions::{Kept, Origin, SavedAt, VersionId, Versions};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The bytes an edit produced for `target`, not yet saved. Applying an edit is the caller's work;
/// this is where its result waits for the original to be kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub(crate) target: PathBuf,
    content: Content,
}

/// What a pending save will write: bytes in memory, or a file that holds them (the cut of a
/// recording is too large to hold twice).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Content {
    Bytes(Vec<u8>),
    File(PathBuf),
}

impl Content {
    /// Writes the content from the start of `out`, and says how many bytes it was.
    fn write_to(&self, out: &mut fs::File) -> std::io::Result<u64> {
        match self {
            Content::Bytes(bytes) => out.write_all(bytes).map(|()| bytes.len() as u64),
            Content::File(path) => std::io::copy(&mut fs::File::open(path)?, out),
        }
    }
}

impl Pending {
    /// `bytes` as the new whole content of the existing file `target`.
    pub fn new(target: impl Into<PathBuf>, bytes: Vec<u8>) -> Pending {
        Pending {
            target: target.into(),
            content: Content::Bytes(bytes),
        }
    }

    /// The content of the file `produced` as the new whole content of the existing file `target`.
    /// The file is read when the save writes, and is left where it is: removing it is the
    /// caller's work.
    pub fn from_file(target: impl Into<PathBuf>, produced: impl Into<PathBuf>) -> Pending {
        Pending {
            target: target.into(),
            content: Content::File(produced.into()),
        }
    }

    /// The same content for `target`.
    pub(crate) fn retargeted(self, target: PathBuf) -> Pending {
        Pending { target, ..self }
    }
}

/// A pending save whose original is already kept as `kept`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackedUp {
    target: PathBuf,
    content: Content,
    kept: Kept,
    original: Original,
    versions: Versions,
    at: SavedAt,
}

/// Whether the new directory entry is known to be on disk. The bytes are saved either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// The file and its folder were synced.
    Confirmed,
    /// The file is saved, but syncing its folder failed, which some network and FUSE mounts do:
    /// a power cut right now could bring the old name back.
    Unconfirmed,
}

/// Whether a failed write has changed the person's file yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Touch {
    Untouched,
    Touched,
}

struct Failed {
    error: StoreError,
    touch: Touch,
}

impl Failed {
    fn untouched(error: StoreError) -> Failed {
        Failed {
            error,
            touch: Touch::Untouched,
        }
    }

    fn touched(error: StoreError) -> Failed {
        Failed {
            error,
            touch: Touch::Touched,
        }
    }
}

impl BackedUp {
    /// Only the versions store makes one, after the original is safely copied.
    pub(crate) fn new(
        pending: Pending,
        kept: Kept,
        original: Original,
        versions: Versions,
        at: SavedAt,
    ) -> BackedUp {
        BackedUp {
            target: pending.target,
            content: pending.content,
            kept,
            original,
            versions,
            at,
        }
    }

    /// The version that holds the original.
    pub fn kept(&self) -> &VersionId {
        &self.kept.id
    }

    /// Replaces the original with the new bytes, whole or not at all.
    ///
    /// The bytes go into a temporary file beside it that has the original's mode, owner,
    /// extended attributes and ACLs, are synced, and are renamed over it, so a crash or a full
    /// disk leaves the original as it was. A file with other hard links is written in place
    /// instead, after the backup, so the links stay one file. One save of a path runs at a time
    /// in this process; if the file changed since it was backed up, its present content is kept
    /// first. A save that fails before it touched the file drops the version it kept: the file
    /// is that version. Once the file is renamed, a failing sync of its folder is reported in
    /// [`Written::durability`], not as an error.
    pub fn write_in_place(self) -> Result<Written, StoreError> {
        let _turn = SaveTurn::wait_for(&self.target);
        let dir = self.target.parent().unwrap_or_else(|| Path::new("."));
        sweep_leftovers(dir);
        let mut redundant = Vec::new();
        if self.kept.origin == Origin::Fresh {
            redundant.push(self.kept.id.clone());
        }
        let written = self.keep_what_changed(&mut redundant).and_then(|links| {
            let result = match links {
                Links::Several => self.write_through().map(|()| Durability::Confirmed),
                Links::One => self.replace(),
            };
            match result {
                Ok(durability) => Ok(durability),
                Err(failed) => Err(self.after(failed, &redundant)),
            }
        });
        written.map(|durability| Written {
            target: self.target.clone(),
            kept: self.kept.id.clone(),
            durability,
        })
    }

    /// Keeps the file as it is now when someone else changed it since the backup, and says how
    /// many names it has.
    fn keep_what_changed(&self, redundant: &mut Vec<VersionId>) -> Result<Links, StoreError> {
        let meta =
            fs::metadata(&self.target).map_err(|e| io_error(StoreOp::Stat, &self.target, &e))?;
        let now = Original::of(&meta);
        if !self.original.is_unchanged_in(&now) {
            let kept = self.versions.keep_current(&self.target, self.at)?;
            if kept.origin == Origin::Fresh {
                redundant.push(kept.id);
            }
        }
        Ok(if now.links > 1 {
            Links::Several
        } else {
            Links::One
        })
    }

    /// What a failure leaves behind: nothing, when the file is as it was.
    fn after(&self, failed: Failed, redundant: &[VersionId]) -> StoreError {
        if failed.touch == Touch::Untouched {
            for id in redundant {
                self.versions.forget(id);
            }
        }
        failed.error
    }

    /// The temporary file, renamed over the target.
    fn replace(&self) -> Result<Durability, Failed> {
        let dir = self.target.parent().unwrap_or_else(|| Path::new("."));
        let temp = save_temp_beside(&self.target);
        let renamed = self.write_temp(&temp).and_then(|()| {
            fs::rename(&temp, &self.target).map_err(|e| io_error(StoreOp::Rename, &self.target, &e))
        });
        if let Err(error) = renamed {
            // The temporary file is ours alone; the original was never opened for writing.
            let _gone = fs::remove_file(&temp);
            return Err(Failed::untouched(error));
        }
        Ok(match fs::File::open(dir).and_then(|d| d.sync_all()) {
            Ok(()) => Durability::Confirmed,
            Err(_) => Durability::Unconfirmed,
        })
    }

    fn write_temp(&self, temp: &Path) -> Result<(), StoreError> {
        let mut file = sys::with_mode(fs::OpenOptions::new().write(true).create_new(true), 0o600)
            .open(temp)
            .map_err(|e| io_error(StoreOp::Write, temp, &e))?;
        if let Ok(old) = fs::File::open(&self.target)
            && let Ok(meta) = old.metadata()
        {
            copy_attributes(&old, &file, &meta);
        }
        self.content
            .write_to(&mut file)
            .map_err(|e| io_error(StoreOp::Write, temp, &e))?;
        // After the write: a write by anyone but root clears set-user-id and set-group-id.
        sys::set_mode(&file, self.original.mode)
            .map_err(|e| io_error(StoreOp::Permissions, temp, &e))?;
        file.sync_all()
            .map_err(|e| io_error(StoreOp::Sync, temp, &e))
    }

    /// The bytes written into the file itself, so its other hard links see them. Not atomic: a
    /// failure half way leaves a mixture, which is why the original was kept first.
    fn write_through(&self) -> Result<(), Failed> {
        let target = &self.target;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(target)
            .map_err(|e| Failed::untouched(io_error(StoreOp::Write, target, &e)))?;
        let touched = |op| move |e: std::io::Error| Failed::touched(io_error(op, target, &e));
        let written = self
            .content
            .write_to(&mut file)
            .map_err(touched(StoreOp::Write))?;
        file.set_len(written).map_err(touched(StoreOp::Write))?;
        file.sync_all().map_err(touched(StoreOp::Sync))
    }
}

/// How many names the file being saved has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Links {
    One,
    Several,
}

/// A file that now holds the saved bytes, and the version that holds what it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The file that was replaced.
    pub target: PathBuf,
    /// Its previous content, restorable through [`Versions::restore`](crate::Versions::restore).
    pub kept: VersionId,
    /// Whether the folder was synced after the rename.
    pub durability: Durability,
}
