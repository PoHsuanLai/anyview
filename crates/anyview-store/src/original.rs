//! What a file was when the viewer read it, so a save can tell that someone else has changed it
//! since, and so the replacement keeps its mode and owner.

use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;

/// A file's identity and state at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Original {
    /// The whole `st_mode`: the permission bits, set-user-id and sticky bits included.
    pub(crate) mode: u32,
    pub(crate) links: u64,
    len: u64,
    modified: (i64, i64),
    inode: u64,
}

impl Original {
    pub(crate) fn of(meta: &Metadata) -> Original {
        Original {
            mode: meta.mode() & 0o7777,
            links: meta.nlink(),
            len: meta.len(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            inode: meta.ino(),
        }
    }

    /// Whether `other` is the same file with the same content stamp: same inode, length and
    /// modification time. A change of mode alone is not a change of content.
    pub(crate) fn is_unchanged_in(&self, other: &Original) -> bool {
        self.inode == other.inode && self.len == other.len && self.modified == other.modified
    }
}
