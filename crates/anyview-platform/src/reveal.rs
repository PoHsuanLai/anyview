//! Showing a file in the file manager.

use crate::error::PlatformError;
use anyview_core::FilePath;
use std::future::Future;

/// Select a file in a file manager window.
pub trait Reveal {
    /// Open the folder holding `file` with it selected.
    fn reveal(&self, file: &FilePath) -> impl Future<Output = Result<(), PlatformError>> + Send;

    /// Whether this implementation has the service behind it. An implementation that answers
    /// "not available" to every request says `false`, so the views never offer what it cannot do.
    fn present(&self) -> bool {
        true
    }
}
