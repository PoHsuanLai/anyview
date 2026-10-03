//! How the bytes of a text file are encoded.

use ds_core::word::Word;

/// A text encoding that a byte-order mark or valid UTF-8 identifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TextEncoding {
    /// UTF-8, with or without a byte-order mark.
    Utf8,
    /// UTF-16, little endian (byte-order mark `FF FE`).
    Utf16Le,
    /// UTF-16, big endian (byte-order mark `FE FF`).
    Utf16Be,
}
