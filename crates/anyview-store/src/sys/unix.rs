//! The Unix half of [`super`]: modes, owners and extended attributes from the file system, the
//! no-replace rename, `flock` and `kill(pid, 0)`.

use super::Claimed;
use rustix::fs::{Access, CWD, FlockOperation, RenameFlags, XattrFlags};
use rustix::fs::{access, fgetxattr, flistxattr, flock, fsetxattr, getxattr, renameat_with};
use rustix::io::Errno;
use std::fs::{File, Metadata, OpenOptions, Permissions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, fchown};
use std::path::Path;

/// The extended attribute browsers and download managers record a file's address in.
const ORIGIN_ATTRIBUTE: &str = "user.xdg.origin.url";

/// The most bytes of an address read: longer is not an address a person would read.
const ORIGIN_LIMIT: usize = 4096;

/// The whole `st_mode`'s permission bits, set-user-id and sticky bits included.
pub(crate) fn mode_of(meta: &Metadata) -> u32 {
    meta.mode() & 0o7777
}

/// The permission bits a person reads in a file's details, when the platform has them.
pub(crate) fn permission_bits(meta: &Metadata) -> Option<u32> {
    Some(meta.mode() & 0o777)
}

/// How many names the file has.
pub(crate) fn links_of(meta: &Metadata) -> u64 {
    meta.nlink()
}

/// The file's identity on its volume.
pub(crate) fn identity_of(meta: &Metadata) -> u64 {
    meta.ino()
}

/// The modification time as seconds and nanoseconds since the epoch.
pub(crate) fn modified_of(meta: &Metadata) -> (i64, i64) {
    (meta.mtime(), meta.mtime_nsec())
}

/// Opens with `mode` as the permissions of a file it creates.
pub(crate) fn with_mode(options: &mut OpenOptions, mode: u32) -> &mut OpenOptions {
    options.mode(mode)
}

/// The folder `path` and every missing one above it, readable by its owner alone.
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    // A folder an older version made with the default mode is closed now.
    let _kept = std::fs::set_permissions(path, Permissions::from_mode(0o700));
    Ok(())
}

/// Gives `file` the permission bits `mode`.
pub(crate) fn set_mode(file: &File, mode: u32) -> io::Result<()> {
    file.set_permissions(Permissions::from_mode(mode))
}

/// Gives `to` what `from` (whose metadata is `meta`) carries: owner, extended attributes, then
/// mode last, so a changed ACL cannot leave the mode bits different.
pub(crate) fn copy_attributes(from: &File, to: &File, meta: &Metadata) {
    // Ownership first: changing it clears set-user-id bits the mode below puts back.
    let _kept = fchown(to, Some(meta.uid()), Some(meta.gid()));
    copy_xattrs(from, to);
    let _kept = set_mode(to, mode_of(meta));
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

/// Whether a save in place of `path` (whose metadata is `meta`) is refused: its mode has no write
/// bit at all, or this process may not write it (another owner's file, a read-only mount).
pub(crate) fn is_locked(path: &Path, meta: &Metadata) -> bool {
    meta.mode() & 0o222 == 0 || access(path, Access::WRITE_OK).is_err()
}

/// Where the file at `path` was downloaded from, as the extended attribute records it.
pub(crate) fn origin_of(path: &Path) -> Option<String> {
    let mut value = vec![0_u8; ORIGIN_LIMIT];
    let length = getxattr(path, ORIGIN_ATTRIBUTE, value.as_mut_slice()).ok()?;
    let text = String::from_utf8(value[..length].to_vec()).ok()?;
    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!text.is_empty()).then(|| text.to_owned())
}

/// `from` renamed to `to` only if nothing has that name, decided by the kernel in one step
/// (`renameat2` with `RENAME_NOREPLACE`, `renameatx_np` with `RENAME_EXCL` on macOS).
pub(crate) fn rename_noreplace(from: &Path, to: &Path) -> Claimed {
    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Claimed::Renamed,
        Err(Errno::EXIST) => Claimed::Taken,
        Err(Errno::INVAL | Errno::NOSYS | Errno::OPNOTSUPP) => Claimed::Unsupported,
        Err(error) => Claimed::Failed(error.into()),
    }
}

/// Waits for the exclusive advisory lock on `file`. The system releases it when the file is
/// closed, however the holder exits.
pub(crate) fn lock_exclusive(file: &File) -> io::Result<()> {
    loop {
        match flock(file, FlockOperation::LockExclusive) {
            Ok(()) => return Ok(()),
            Err(Errno::INTR) => {}
            Err(errno) => return Err(errno.into()),
        }
    }
}

/// Whether process `pid` exists. A process we may not signal still exists.
pub(crate) fn is_running(pid: i32) -> bool {
    let Some(pid) = rustix::process::Pid::from_raw(pid) else {
        return true;
    };
    !matches!(
        rustix::process::test_kill_process(pid),
        Err(rustix::io::Errno::SRCH)
    )
}
