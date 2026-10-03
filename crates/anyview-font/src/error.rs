//! The one error `anyview-font` returns, with variants a caller acts on.

use anyview_core::{ByteLen, FormatKind};
use std::path::PathBuf;

/// Why a font could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FontError {
    /// The disk refused to read the file.
    #[error("cannot read {path:?}: {kind}")]
    Read {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        kind: std::io::ErrorKind,
    },
    /// The file's sniffed kind belongs to another peek.
    #[error("a {kind:?} file is not what this peek reads")]
    WrongKind {
        /// What the file was sniffed as.
        kind: FormatKind,
    },
    /// The font is longer than the bytes a peek may read: its tables are spread over the whole
    /// file, so it is read whole or not at all.
    #[error("the font is {} bytes and the preview may read {}", .len.0, .allowed.0)]
    OverBudget {
        /// The file's length.
        len: ByteLen,
        /// The most the peek may read.
        allowed: ByteLen,
    },
    /// The bytes are not a font.
    #[error("not a valid font: {reason}")]
    Malformed {
        /// The parser's own words.
        reason: String,
    },
}
