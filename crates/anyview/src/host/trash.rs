//! Moving a file to the trash: the freedesktop trash on Linux, behind a trait so a test can see
//! what was trashed without touching the person's.

use anyview_core::FilePath;

/// The trash refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot move {path:?} to the trash: {reason}")]
pub struct TrashError {
    /// The file.
    pub path: FilePath,
    /// What the trash library reported.
    pub reason: String,
}

/// Where a trashed file goes.
pub trait Trash: Send + Sync + 'static {
    /// Move `file` to the trash. Blocking.
    fn trash(&self, file: &FilePath) -> Result<(), TrashError>;
}

/// The desktop's own trash, through the `trash` crate.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTrash;

impl Trash for SystemTrash {
    fn trash(&self, file: &FilePath) -> Result<(), TrashError> {
        trash::delete(file.as_path()).map_err(|error| TrashError {
            path: file.clone(),
            reason: error.to_string(),
        })
    }
}
