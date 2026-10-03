//! The platform edge: what the viewer needs from the desktop around it, each as a trait with a
//! Linux implementation (`linux`) and a fake (`testing`, behind the `testing` feature). The
//! traits are single instance, the media session, Open With, the thumbnail cache, printing,
//! sharing, revealing a file and window stacking. This is the only crate that names D-Bus or the
//! freedesktop file formats; the directories, the session bus and the way a program is started
//! come in through [`Env`].
//!
//! Every public item is reached from this root once, except the implementations, which are
//! reached through `linux` and `testing`. Another platform adds a module beside `linux` and
//! changes nothing outside this crate.

mod apps;
mod env;
mod error;
mod instance;
mod media;
mod printer;
mod reveal;
mod share;
mod spawn;
mod stacking;
mod thumbnail;
mod uri;

pub mod linux;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use apps::{AppEntry, AppsForType, Association, DesktopId};
pub use env::{BusRoute, Dirs, Env};
pub use error::{IoOp, PlatformError};
pub use instance::{Claim, Handoff, Instance, Primary, Request};
pub use media::{
    Ability, MediaControl, MediaSession, MediaState, PlaybackStatus, SeekDirection, TrackSerial,
};
pub use printer::{JobTitle, PrintOutcome, Printer};
pub use reveal::Reveal;
pub use share::{Share, ShareTarget};
pub use spawn::{Argv, ProcessSpawn, RefuseSpawn, Spawn};
pub use stacking::{Stacking, StackingOutcome, WindowStacking};
pub use thumbnail::{ThumbPixels, ThumbSize, ThumbnailCache};
pub use uri::file_uri;
