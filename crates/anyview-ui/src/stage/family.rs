//! Which stage a file gets.

use ds_core::word::Word;

/// The family of stage that shows a file; decided once a file has been sniffed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StageFamily {
    /// Images: fit, zoom, pan, rotate.
    Raster,
    /// PDF documents: pages, find, jump.
    Pdf,
    /// Video and audio.
    Media,
    /// Text, code and Markdown.
    Text,
    /// No stage: the file shows its facts and Open With… (`StageSupport::PeekOnly`).
    PeekOnly,
}
