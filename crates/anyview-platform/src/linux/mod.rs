//! The Linux implementations: freedesktop specifications and D-Bus. A new platform adds a
//! sibling module with the same shapes and selects it in `lib.rs`.

mod apps;
mod instance;
mod mpris;
mod print;
mod reveal;
mod share;
mod stacking;
mod thumbnails;

pub use apps::DesktopApps;
pub use instance::{BUS_NAME, DbusInstance};
pub use mpris::{MPRIS_NAME, MprisSession};
pub use print::PortalPrinter;
pub use reveal::FileManagerReveal;
pub use share::MailShare;
pub use stacking::NoStacking;
pub use thumbnails::FreedesktopThumbnails;
