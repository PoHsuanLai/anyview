//! The half of [`super`] for a platform with no Unix modes, owners or extended attributes
//! (Windows): the read-only flag stands for the mode, a hard link claims a name, and the lock is
//! the standard library's.

use super::Claimed;
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// The mode a file with no Unix permissions stands for: read-only has no write bit.
const READ_ONLY: u32 = 0o444;
const WRITABLE: u32 = 0o666;

/// The permission bits of `meta`: the read-only flag as a mode.
pub(crate) fn mode_of(meta: &Metadata) -> u32 {
    if meta.permissions().readonly() {
        READ_ONLY
    } else {
        WRITABLE
    }
}

/// Windows has no permission bits to show a person.
pub(crate) fn permission_bits(_meta: &Metadata) -> Option<u32> {
    None
}

/// The standard library does not say how many names a file has here: one, which is saved by
/// replacing it.
pub(crate) fn links_of(_meta: &Metadata) -> u64 {
    1
}

/// The standard library does not give a file's identity here, so a file is told by its length and
/// its modification time alone.
pub(crate) fn identity_of(_meta: &Metadata) -> u64 {
    0
}

/// The modification time as seconds and nanoseconds since the epoch.
pub(crate) fn modified_of(meta: &Metadata) -> (i64, i64) {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or((0, 0), |since| {
            (
                i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
                i64::from(since.subsec_nanos()),
            )
        })
}

/// No modes here: the file is created as any other.
pub(crate) fn with_mode(options: &mut OpenOptions, _mode: u32) -> &mut OpenOptions {
    options
}

/// The folder `path` and every missing one above it. A folder under the person's profile is
/// private to them already.
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Gives `file` the read-only flag when `mode` has no write bit.
pub(crate) fn set_mode(file: &File, mode: u32) -> io::Result<()> {
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(mode & 0o222 == 0);
    file.set_permissions(permissions)
}

/// Gives `to` the read-only flag `from` has. There are no owners or extended attributes to keep.
pub(crate) fn copy_attributes(_from: &File, to: &File, meta: &Metadata) {
    let _kept = set_mode(to, mode_of(meta));
}

/// Whether a save in place of `path` is refused: the file is marked read-only.
pub(crate) fn is_locked(_path: &Path, meta: &Metadata) -> bool {
    meta.permissions().readonly()
}

/// The address the file was downloaded from, which Windows keeps in the `Zone.Identifier` stream
/// of the file: `HostUrl=` (the file's own address), else `ReferrerUrl=` (the page it was on).
pub(crate) fn origin_of(path: &Path) -> Option<String> {
    let mut stream = path.as_os_str().to_os_string();
    stream.push(":Zone.Identifier");
    let bytes = std::fs::read(stream).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let value_of = |key: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(key))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    value_of("HostUrl=").or_else(|| value_of("ReferrerUrl="))
}

/// No call decides a no-replace rename in one step here; a hard link claims the name instead.
pub(crate) fn rename_noreplace(_from: &Path, _to: &Path) -> Claimed {
    Claimed::Unsupported
}

/// Waits for the exclusive lock on `file`, which the system releases when the file is closed.
pub(crate) fn lock_exclusive(file: &File) -> io::Result<()> {
    file.lock()
}

/// Whether process `pid` exists: not told here, so a leftover is never taken for abandoned.
pub(crate) fn is_running(_pid: i32) -> bool {
    true
}
