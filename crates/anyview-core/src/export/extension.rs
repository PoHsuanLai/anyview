//! The file extensions an export writes.

use ds_core::word::Word;

/// The extension of a file an export writes: a closed set, because only formats the viewer can
/// encode appear. The slug is the extension without its dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ExportExtension {
    /// `.png`
    Png,
    /// `.jpg`
    Jpg,
    /// `.webp`
    Webp,
    /// `.avif`
    Avif,
    /// `.tiff`
    Tiff,
    /// `.pdf`
    Pdf,
    /// `.txt`
    Txt,
    /// `.md`
    Md,
}
