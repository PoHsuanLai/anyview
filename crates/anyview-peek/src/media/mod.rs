//! The peek of video and audio, in one of two forms chosen by the `media` feature: pure-Rust
//! parsers read a recording's header (symphonia for audio, mp4parse for MP4 and MOV,
//! matroska-demuxer for Matroska and WebM), or (feature off) the recording is described like any
//! kind with no back end. Either way the module exports `VideoPeek`, `AudioPeek` and `MediaLook`, so
//! the rest of the crate names no feature.

#[cfg(feature = "media")]
mod audio;
#[cfg(feature = "media")]
mod matroska;
#[cfg(feature = "media")]
mod mp4;
#[cfg(feature = "media")]
mod peek;
#[cfg(feature = "media")]
mod recording;
#[cfg(feature = "media")]
pub use peek::{AudioPeek, MediaLook, VideoPeek};

#[cfg(not(feature = "media"))]
mod absent;
#[cfg(not(feature = "media"))]
pub use absent::{AudioPeek, MediaLook, VideoPeek};
