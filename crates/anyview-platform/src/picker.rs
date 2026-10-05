//! Choosing a file to open through the desktop's own dialog.

use crate::error::PlatformError;
use anyview_core::FilePath;
use std::future::Future;

/// How a request for a file ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickOutcome {
    /// The person chose these files, in the order the dialog listed them.
    Chosen(Vec<FilePath>),
    /// The person closed the dialog.
    Cancelled,
    /// There is no file dialog to show.
    NoDialog,
}

/// Choose a file.
pub trait Picker {
    /// Show the dialog and answer what the person chose.
    fn pick(&self) -> impl Future<Output = Result<PickOutcome, PlatformError>> + Send;
}
