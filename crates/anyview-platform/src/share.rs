//! Sending a file to someone. Copying the file to the clipboard stays with the window, whose
//! clipboard must outlive the call.

use crate::error::PlatformError;
use anyview_core::FilePath;
use ds_core::word::Word;
use std::future::Future;

/// Where a file can be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ShareTarget {
    /// A new message in the mail program with the file attached.
    Mail,
}

/// Send files out of the viewer.
pub trait Share {
    /// The targets this desktop can send to.
    fn targets(&self) -> Vec<ShareTarget>;

    /// Send `file` to `target`.
    fn share(
        &self,
        file: &FilePath,
        target: ShareTarget,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send;
}
