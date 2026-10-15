//! What a person's file carries besides its bytes, kept across a save and a copy the way a mature
//! editor keeps it: the owner where we may, the extended attributes (which hold POSIX ACLs and
//! labels), and the mode. Best effort everywhere: a file system that has none of these is no
//! reason to lose the bytes. What the platform has is `sys`'s to say.

use crate::sys;
use std::fs::{self, Metadata};
use std::path::Path;

pub(crate) use crate::sys::copy_attributes;

/// Whether a save in place of `path` is refused: its mode has no write bit at all, or this
/// process may not write it (another owner's file, a read-only mount). A person unlocks a file
/// with its permissions, as in any editor; a window hides or disables edits of one that is.
/// A file that cannot be looked at is not read-only: opening it fails with its own error.
pub fn is_read_only(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    is_locked(path, &meta)
}

pub(crate) fn is_locked(path: &Path, meta: &Metadata) -> bool {
    sys::is_locked(path, meta)
}
