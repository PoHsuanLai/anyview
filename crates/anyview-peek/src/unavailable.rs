//! Why a file has no preview, in the terms a host acts on.

use crate::error::PeekError;
use anyview_core::ByteLen;

/// Why a peek could not be made. The card still lists the file's type, size and date; the pane
/// says [`label`](Unavailable::label) beside them, and a host picks its own words from the
/// variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// The file is not there.
    Missing,
    /// The file is there and cannot be read: no permission, a device or a socket, a folder that
    /// cannot be listed, or bytes that fail to read.
    Unreadable,
    /// The peek has to read more of the file than its budget allows.
    TooBig {
        /// The file's length.
        len: ByteLen,
        /// The most the peek may read.
        allowed: ByteLen,
    },
    /// The file is not what it says, or is damaged, or its back end gave up; this says why.
    Damaged(String),
}

impl Unavailable {
    /// The words the pane shows for this: one sentence, naming no path.
    pub fn label(&self) -> String {
        match self {
            Unavailable::Missing => "the file does not exist".to_owned(),
            Unavailable::Unreadable => "the file cannot be read".to_owned(),
            Unavailable::TooBig { len, allowed } => format!(
                "the file is {} bytes and the preview may read {}",
                len.0, allowed.0
            ),
            Unavailable::Damaged(reason) => reason.clone(),
        }
    }

    /// What `error` comes to for a card.
    pub(crate) fn of(error: &PeekError) -> Self {
        match error {
            PeekError::Missing { .. } => Unavailable::Missing,
            PeekError::Unreadable { .. } | PeekError::Folder { .. } => Unavailable::Unreadable,
            PeekError::OverBudget { len, allowed } => Unavailable::TooBig {
                len: *len,
                allowed: *allowed,
            },
            PeekError::Image(_)
            | PeekError::Text(_)
            | PeekError::Media { .. }
            | PeekError::Archive(_)
            | PeekError::Font(_)
            | PeekError::Pdf(_)
            | PeekError::WrongKind { .. } => Unavailable::Damaged(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn an_error_becomes_the_reason_a_host_acts_on_and_names_no_path() {
        let path = PathBuf::from("/home/ann/secret.png");
        let missing = Unavailable::of(&PeekError::Missing { path: path.clone() });
        let refused = Unavailable::of(&PeekError::Unreadable {
            path,
            kind: std::io::ErrorKind::PermissionDenied,
        });
        let big = Unavailable::of(&PeekError::OverBudget {
            len: ByteLen(621),
            allowed: ByteLen(100),
        });
        let damaged = Unavailable::of(&PeekError::media("no moov"));
        // name, reason, words
        let cases = [
            ("missing", missing, "the file does not exist"),
            ("refused", refused, "the file cannot be read"),
            (
                "over budget",
                big,
                "the file is 621 bytes and the preview may read 100",
            ),
            ("damaged", damaged, "cannot read the recording: no moov"),
        ];
        for (name, reason, words) in cases {
            assert_eq!(reason.label(), words, "{name}");
        }
    }
}
