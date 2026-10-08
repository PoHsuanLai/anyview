//! How large the window of an audio file is: it holds a player and little else, so it opens small,
//! as QuickTime's audio-only window does: a cover (or the music tile) over the title.

use anyview_core::{PixelLen, PixelSize};

/// The window of an audio file: 360 wide and 460 high, logical pixels. The cover is at most 320
/// across, with the title and the artist below it.
const AUDIO: (u32, u32) = (360, 460);

/// The size an audio file's window opens at, in logical pixels.
pub fn audio_window_size() -> PixelSize {
    PixelSize {
        width: PixelLen(AUDIO.0),
        height: PixelLen(AUDIO.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_audio_window_is_small_and_taller_than_wide() {
        let size = audio_window_size();
        assert_eq!((size.width.0, size.height.0), (360, 460));
    }
}
