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
    /// `.m4a`
    M4a,
    /// `.mp3`
    Mp3,
    /// `.flac`
    Flac,
    /// `.wav`
    Wav,
    /// `.opus`
    Opus,
    /// The extension that goes with what the file holds, which only the back end knows: a trim
    /// keeps its source's, and a copied audio track takes its codec's. It is never written as a
    /// name.
    Matching,
}
