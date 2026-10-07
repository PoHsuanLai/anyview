//! The Linux desktop's own services: D-Bus (the session bus, the portals, MPRIS, the file
//! manager) and the freedesktop file formats (application entries). Built with the
//! `quire-desktop` feature on Linux. A new platform adds a sibling module with the same shapes
//! and selects it in `lib.rs`.
//!
//! The parts of the old Linux module that call no desktop service (the thumbnail cache, `xdg-open`,
//! window stacking) now live in `portable` and are re-exported here under their old names.

mod apps;
mod instance;
mod mpris;
mod picker;
mod portal;
mod print;
mod reveal;
mod share;

pub use crate::portable::{FreedesktopThumbnails, NoStacking, SystemOpen as XdgOpen};
pub use apps::DesktopApps;
pub use instance::{BUS_NAME, DbusInstance, forward_over};
pub use mpris::{MPRIS_NAME, MprisSession};
pub use picker::PortalPicker;
pub use print::PortalPrinter;
pub use reveal::FileManagerReveal;
pub use share::MailShare;
