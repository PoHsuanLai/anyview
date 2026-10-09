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

/// The files a dialog offers to start with: the media types and the file name patterns of what
/// the viewer can show. A dialog offers them as its first, chosen filter ("Supported files") and
/// keeps an "All files" one beside it, so a file outside the list can still be chosen. A dialog
/// that has no filters, or an empty set, lists everything.
///
/// The desktop's dialogs hide the files a filter does not match, rather than greying them out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileKinds {
    /// Media types, by their registry names (`image/png`). A type's subtypes match too.
    pub mimes: Vec<String>,
    /// File name patterns, for the formats a media type cannot name (`*.jsonl`).
    pub globs: Vec<String>,
}

impl FileKinds {
    /// Whether the set names nothing, so the dialog lists everything.
    pub fn is_empty(&self) -> bool {
        self.mimes.is_empty() && self.globs.is_empty()
    }
}

/// Choose a file.
pub trait Picker {
    /// Show the dialog, offering `kinds` first, and answer what the person chose.
    fn pick(
        &self,
        kinds: &FileKinds,
    ) -> impl Future<Output = Result<PickOutcome, PlatformError>> + Send;

    /// Whether this implementation has the service behind it. An implementation that answers
    /// "not available" to every request says `false`, so the views never offer what it cannot do.
    fn present(&self) -> bool {
        true
    }
}
