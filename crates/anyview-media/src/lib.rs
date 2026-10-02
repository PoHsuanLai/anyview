//! Video and audio for the viewer. Two C libraries sit behind two features, so the launcher can
//! link the one it needs: `player` is libmpv drawing into a texture the caller owns (a typestate
//! `Session`, and the `Driver` an actor runs), and `ffmpeg` is libav reading a recording's facts
//! and cover art and writing trims, extracted audio and conversions. This is the only crate that
//! names either library. Nothing here spawns a thread or reads a clock: the binary runs the
//! driver on its media thread and the exports on its pool.
//!
//! Every public item is reached from this root, once.

mod command;
mod error;
mod event;
#[cfg(feature = "ffmpeg")]
mod export;
#[cfg(feature = "ffmpeg")]
mod libav;
#[cfg(feature = "ffmpeg")]
mod probe;

#[cfg(feature = "player")]
mod device;
#[cfg(feature = "player")]
mod driver;
#[cfg(feature = "player")]
mod session;

pub use command::{Direction, MediaCommand, Pace, PictureSlot, ShotContent};
#[cfg(feature = "player")]
pub use device::headless_device;
#[cfg(feature = "player")]
pub use driver::{Continuation, Driver, FrameSink, Handled};
pub use error::MediaError;
pub use event::{EndReason, MediaEvent};
#[cfg(feature = "ffmpeg")]
pub use export::{
    AudioFormat, Encoder, Encoders, ExportBackend, ExportProgress, ExportReport, ExportRequest,
    ProgressSink, output_path, plan_export,
};
#[cfg(feature = "ffmpeg")]
pub use probe::{
    AudioFacts, AudioPeek, CoverArt, CoverCodec, MediaPeeked, MediaProbe, VideoFacts, VideoPeek,
    probe, probe_within,
};
#[cfg(feature = "player")]
pub use session::{AudioDriver, Frame, Idle, Loaded, Opened, Opening, Refused, Report, Session};
/// The texture type the player draws into, named so a [`FrameSink`] can be written without
/// depending on `wgpu` itself.
#[cfg(feature = "player")]
pub use wgpu::{Device, Queue, TextureView};
