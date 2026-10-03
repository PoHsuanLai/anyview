//! The peek of video and audio, in one of two forms chosen by the `media` feature: libav reads a
//! recording's facts and cover, or (feature off) the recording is described like any kind with no
//! back end. Either way the module exports `VideoPeek`, `AudioPeek` and `MediaLook`, so the rest of
//! the crate names no feature.

#[cfg(feature = "media")]
mod libav;
#[cfg(feature = "media")]
pub use libav::{AudioPeek, MediaLook, VideoPeek};

#[cfg(not(feature = "media"))]
mod absent;
#[cfg(not(feature = "media"))]
pub use absent::{AudioPeek, MediaLook, VideoPeek};
