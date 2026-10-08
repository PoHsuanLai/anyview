//! A recording's peek without the header readers: what sniffing established, plus the size and date every pane
//! lists. No duration, no codec, no cover; nothing is pretended.

use crate::described::{Described, Describes, FactsPeek};
use anyview_core::{FormatKind, Input, PixelSize, Sniffed};

/// What a recording's peek holds without the header readers: the words for the file's type.
pub type MediaLook = Described;

/// The kind `Video`.
#[derive(Debug, Clone, Copy)]
pub struct VideoKind;

impl Describes for VideoKind {
    const KIND: FormatKind = FormatKind::Video;
}

/// The facts-only peek of a video file.
pub type VideoPeek = FactsPeek<VideoKind>;

/// The kind `Audio`.
#[derive(Debug, Clone, Copy)]
pub struct AudioKind;

impl Describes for AudioKind {
    const KIND: FormatKind = FormatKind::Audio;
}

/// The facts-only peek of an audio file.
pub type AudioPeek = FactsPeek<AudioKind>;

/// Without the header readers a recording has no size to give.
pub fn video_size(_src: &Input, _sniffed: &Sniffed) -> Option<PixelSize> {
    None
}
