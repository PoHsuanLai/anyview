//! The peek of video and audio, in one of two forms chosen by the `media` feature: pure-Rust
//! parsers read a recording's header (symphonia for audio, mp4parse for MP4 and MOV,
//! matroska-demuxer for Matroska and WebM), or (feature off) the recording is described like any
//! kind with no back end. Either way the module exports `VideoPeek`, `AudioPeek` and `MediaLook`, so
//! the rest of the crate names no feature.

#[cfg(feature = "media")]
mod audio;
mod cover;
#[cfg(feature = "media")]
mod matroska;
#[cfg(feature = "media")]
mod mp4;
#[cfg(feature = "media")]
mod peek;
#[cfg(feature = "media")]
mod recording;
#[cfg(feature = "media")]
pub use peek::{AudioPeek, MediaLook, VideoPeek, audio_cover, video_size};

/// A stream over `src` at its start, once its first byte has read: a missing file, a FIFO or a
/// failing source is [`PeekError::Unreadable`] here, not a parser's complaint later.
#[cfg(feature = "media")]
fn opened(src: &anyview_core::Input) -> Result<anyview_core::ReadAtStream, crate::PeekError> {
    src.bytes()
        .read_at(0, &mut [0u8; 1])
        .map_err(|error| crate::PeekError::Unreadable {
            path: src.label(),
            kind: error.kind(),
        })?;
    Ok(src.reader())
}

#[cfg(not(feature = "media"))]
mod absent;
#[cfg(not(feature = "media"))]
pub use absent::{AudioPeek, MediaLook, VideoPeek, audio_cover, video_size};

pub use cover::AudioCover;
