//! What a person's file carries besides its bytes, kept across a save and a copy the way a mature
//! editor keeps it: the owner where we may, the extended attributes (which hold POSIX ACLs and
//! labels), and the mode. Best effort everywhere: a file system that has none of these is no
//! reason to lose the bytes.

use rustix::fs::{Access, XattrFlags, access, fgetxattr, flistxattr, fsetxattr};
use std::fs::{self, File, Metadata, Permissions};
use std::os::unix::fs::{MetadataExt, PermissionsExt, fchown};
use std::path::Path;

/// Gives `to` what `from` (whose metadata is `meta`) carries: owner, extended attributes, then
/// mode last, so a changed ACL cannot leave the mode bits different.
pub(crate) fn copy_attributes(from: &File, to: &File, meta: &Metadata) {
    // Ownership first: changing it clears set-user-id bits the mode below puts back.
    let _kept = fchown(to, Some(meta.uid()), Some(meta.gid()));
    copy_xattrs(from, to);
    let _kept = to.set_permissions(Permissions::from_mode(meta.mode() & 0o7777));
}

fn copy_xattrs(from: &File, to: &File) {
    let probe: &mut [u8] = &mut [];
    let Ok(size) = flistxattr(from, probe) else {
        return;
    };
    let mut names = vec![0_u8; size];
    let Ok(size) = flistxattr(from, names.as_mut_slice()) else {
        return;
    };
    for name in names[..size]
        .split(|byte| *byte == 0)
        .filter(|n| !n.is_empty())
    {
        let Ok(name) = std::ffi::CString::new(name) else {
            continue;
        };
        let probe: &mut [u8] = &mut [];
        let Ok(length) = fgetxattr(from, name.as_c_str(), probe) else {
            continue;
        };
        let mut value = vec![0_u8; length];
        let Ok(length) = fgetxattr(from, name.as_c_str(), value.as_mut_slice()) else {
            continue;
        };
        let _kept = fsetxattr(to, name.as_c_str(), &value[..length], XattrFlags::empty());
    }
}

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
    meta.mode() & 0o222 == 0 || access(path, Access::WRITE_OK).is_err()
}
