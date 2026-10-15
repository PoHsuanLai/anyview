//! What a file was when the viewer read it, so a save can tell that someone else has changed it
//! since, and so the replacement keeps its mode and owner.

use crate::sys;
use std::fs::Metadata;

/// A file's identity and state at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Original {
    /// The permission bits (on Unix the whole `st_mode`, set-user-id and sticky bits included).
    pub(crate) mode: u32,
    pub(crate) links: u64,
    len: u64,
    modified: (i64, i64),
    inode: u64,
}

impl Original {
    pub(crate) fn of(meta: &Metadata) -> Original {
        Original {
            mode: sys::mode_of(meta),
            links: sys::links_of(meta),
            len: meta.len(),
            modified: sys::modified_of(meta),
            inode: sys::identity_of(meta),
        }
    }

    /// Whether `other` is the same file with the same content stamp: same inode, length and
    /// modification time. A change of mode alone is not a change of content.
    pub(crate) fn is_unchanged_in(&self, other: &Original) -> bool {
        self.inode == other.inode && self.len == other.len && self.modified == other.modified
    }
}
