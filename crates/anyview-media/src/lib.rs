//! Video and audio for the viewer. Playing is mpv, the person's own, run as a child process with
//! mpv-wgpu's C plugin loaded into it, drawing into a texture the caller owns (a typestate
//! `Session`, and the `Driver` an actor runs); feature `player`. With no mpv, audio plays through
//! the built-in player (`BuiltinDriver`: symphonia decodes, cpal plays); feature `audio`. Both are
//! a `MediaDriver`, which is all the media thread knows. Writing a trim, extracted audio
//! or a conversion is the FFmpeg plugin's: this crate plans and names the work (`plan_export`,
//! `ask_of`, `output_path`) and the binary asks the plugin. No library of codecs is linked and
//! nothing here spawns a thread or reads a clock: the binary runs the driver on its media thread
//! and the exports on its pool.
//!
//! Every public item is reached from this root, once.
//!
//! Planning needs neither feature:
//!
//! ```
//! use anyview_core::{ExportJob, FilePath, MediaExport, RasterTarget};
//! use anyview_media::plan_export;
//!
//! let movie = FilePath::new("/home/me/clip.mkv")?;
//! let jobs = plan_export(&movie, MediaExport::CurrentFrame(RasterTarget::Png));
//! assert!(matches!(jobs.as_slice(), [ExportJob::MpvScreenshot { .. }]));
//! # Ok::<(), anyview_core::CoreError>(())
//! ```

#![warn(missing_docs)]

mod command;
mod error;
mod event;
mod export;
mod playing;

#[cfg(feature = "audio")]
mod builtin;

#[cfg(feature = "player")]
mod device;
#[cfg(feature = "player")]
mod driver;
#[cfg(feature = "player")]
mod session;

#[cfg(feature = "audio")]
pub use builtin::{
    BuiltinDriver, CardOutput, Gate, Pipe, SilentOutput, SoundOutput, StreamFormat, card_present,
    playable,
};
pub use command::{Direction, MediaCommand, Pace, PictureSlot, ShotContent};
#[cfg(feature = "player")]
pub use device::{GPU_OPENING, headless_device};
#[cfg(feature = "player")]
pub use driver::{Driver, FrameSink};
pub use error::MediaError;
pub use event::{Abilities, Ability, EndReason, MediaEvent};
pub use export::{
    Ask, ExportProgress, ExportReport, ExportRequest, NameHints, ProgressSink, ask_of,
    offered_kinds, output_path, plan_export, target_of,
};
pub use playing::{Continuation, Handled, MediaDriver};
#[cfg(feature = "player")]
pub use session::{
    AudioDriver, Frame, Idle, Loaded, MpvHost, Opened, Opening, Refused, Report, Session,
};
/// The texture type the player draws into, named so a [`FrameSink`] can be written without
/// depending on `wgpu` itself.
#[cfg(feature = "player")]
pub use wgpu::{Device, Queue, TextureView};
