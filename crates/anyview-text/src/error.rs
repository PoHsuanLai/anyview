//! The one error `anyview-text` returns, with variants a caller acts on.

use anyview_core::{FormatKind, LineIndex};
use std::path::PathBuf;

/// Why text could not be read, windowed, highlighted or parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
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
    /// A peek budget that allows no byte.
    #[error("the peek budget allows no bytes")]
    NoBudget,
    /// A JSON value that does not parse.
    #[error("invalid JSON at line {}, column {column}: {reason}", .line.0 + 1)]
    Json {
        /// The line, zero-based.
        line: LineIndex,
        /// The column on that line, one-based.
        column: u32,
        /// The parser's own words.
        reason: String,
    },
    /// A JSON file too large for a peek to parse as one value.
    #[error("a JSON document larger than the peek budget is not parsed")]
    JsonOverBudget,
    /// A delimited table that does not parse.
    #[error("invalid table: {reason}")]
    Table {
        /// The parser's own words.
        reason: String,
    },
    /// A tree path that leads nowhere.
    #[error("no node at that path")]
    NoSuchNode,
}
