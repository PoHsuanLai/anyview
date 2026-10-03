//! The one error `anyview-core` returns: a value handed in at a boundary that does not name
//! anything the viewer can hold.

use std::path::PathBuf;

/// Why a constructor refused its input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    /// A file path was relative; a source is always an absolute path.
    #[error("a file path must be absolute: {path:?}")]
    PathNotAbsolute {
        /// The path as given.
        path: PathBuf,
    },
    /// A file name was empty, a dot entry, or held a separator or NUL.
    #[error("not a file name: {name:?}")]
    NameInvalid {
        /// The name as given.
        name: String,
    },
    /// A MIME type was not `type/subtype`.
    #[error("not a MIME type: {text:?}")]
    MimeInvalid {
        /// The text as given.
        text: String,
    },
    /// A syntax name was empty or held a character outside `a-z 0-9 + # - _ .`.
    #[error("not a syntax name: {text:?}")]
    SyntaxNameInvalid {
        /// The text as given.
        text: String,
    },
    /// A page range ended before it started.
    #[error("a page range cannot end at page {end} before it starts at page {start}")]
    PageRangeReversed {
        /// The first page, zero-based.
        start: u32,
        /// The last page, zero-based.
        end: u32,
    },
    /// A time range that ends at or before its start.
    #[error("a time range cannot end at {end} us before it starts at {start} us")]
    TimeRangeEmpty {
        /// Where it starts, in microseconds.
        start: u64,
        /// Where it ends, in microseconds.
        end: u64,
    },
    /// A resolution outside `1..=2400` dpi.
    #[error("a resolution must be 1 to 2400 dpi, not {value}")]
    DpiOutOfRange {
        /// The value as given.
        value: u32,
    },
    /// A sequence was asked to start at a file it does not hold.
    #[error("{path:?} is not in the sequence")]
    NotInSequence {
        /// The file that was looked for.
        path: PathBuf,
    },
}
