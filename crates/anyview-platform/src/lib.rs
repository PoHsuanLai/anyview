//! The platform edge: what the viewer needs from the desktop around it, each as a trait with a
//! Linux implementation (`linux`) and a fake (`testing`, behind the `testing` feature). The
//! traits are single instance, the media session, Open With, the thumbnail cache, printing,
//! sharing, revealing a file, choosing a file, opening a web link and window stacking. This is the
//! only crate that names D-Bus or the freedesktop file formats; the directories, the session bus
//! and the way a program is started come in through [`Env`].
//!
//! Two implementation modules. `portable` is built everywhere and calls no desktop service: single
//! instance over a per-user socket, the platform's opener, the thumbnail cache, and the traits'
//! "not available" answers. `linux` is the Linux desktop's own services over D-Bus and the
//! freedesktop formats, built on Linux with the `quire-desktop` feature (on by default); without
//! the feature this crate does not depend on zbus.
//!
//! Every public item is reached from this root once, except the implementations, which are
//! reached through `portable`, `linux` and `testing`. Another platform adds a module beside
//! `linux` and changes nothing outside this crate.

mod apps;
mod env;
mod error;
mod handoff;
mod instance;
mod link;
mod media;
mod picker;
mod plugin;
mod printer;
mod reveal;
mod share;
mod spawn;
mod stacking;
mod thumbnail;
mod uri;

#[cfg(all(feature = "quire-desktop", target_os = "linux"))]
pub mod linux;
pub mod portable;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use apps::{AppEntry, AppsForType, Association, DesktopId};
pub use env::{BusRoute, Dirs, Env};
pub use error::{IoOp, PlatformError};
pub use instance::{Claim, Handoff, Instance, Primary, Request};
pub use link::OpenLink;
pub use media::{
    Ability, MediaControl, MediaSession, MediaState, PlaybackStatus, SeekDirection, TrackSerial,
};
pub use picker::{PickOutcome, Picker};
pub use plugin::{Discovery, PluginFacts, PluginRunner, Rejected, Timeouts, discover};
pub use printer::{JobTitle, PrintOutcome, Printer};
pub use reveal::Reveal;
pub use share::{Share, ShareTarget};
pub use spawn::{Argv, ProcessSpawn, RefuseSpawn, Spawn};
pub use stacking::{Stacking, StackingOutcome, WindowStacking};
pub use thumbnail::{ThumbPixels, ThumbSize, ThumbnailCache};
pub use uri::file_uri;
