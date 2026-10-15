//! What differs between a Unix and Windows: modes and owners, extended attributes, the claim on a
//! name that is decided in one step, the lock on a store and whether a process is alive. Each is
//! one small function the rest of the crate calls, so only this folder names `std::os::unix` or
//! `rustix`. On Windows the things that have no meaning there (an owner, a mode, an ACL as
//! extended attributes) are left alone, and the rest is done with the standard library.

#[cfg(unix)]
mod unix;
#[cfg(not(unix))]
mod windows;

#[cfg(unix)]
pub(crate) use unix::*;
#[cfg(not(unix))]
pub(crate) use windows::*;

/// How a rename that must not replace a file went, when the platform can ask the file system to
/// decide in one step.
#[derive(Debug)]
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) enum Claimed {
    /// The file now has the new name.
    Renamed,
    /// A file has the new name already.
    Taken,
    /// This file system or platform cannot decide it in one step: the caller claims the name another way.
    Unsupported,
    /// The rename failed for another reason.
    Failed(std::io::Error),
}
