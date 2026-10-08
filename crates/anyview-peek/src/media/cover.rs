//! The cover an audio file carries, as the viewer shows it.

use anyview_core::PixelSize;

/// The picture an audio file carries of itself, reduced to what a window shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioCover {
    /// The picture as PNG bytes, reduced to the budget it was read with.
    pub png: Vec<u8>,
    /// The size the file's own picture has, before it was reduced.
    pub size: PixelSize,
}
