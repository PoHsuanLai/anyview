//! The Linux desktop's own services (quire design/36's `desktop/` module): D-Bus (the session bus, the portals, MPRIS, the file
//! manager) and the freedesktop file formats (application entries). Built with the
//! `quire-desktop` feature on Linux. A new platform adds a sibling module with the same shapes
//! and selects it in `lib.rs`.
//!
//! The parts of the old Linux module that call no desktop service (the thumbnail cache, `xdg-open`,
//! window stacking) now live in `portable` and are re-exported here under their old names.

mod apps;
mod env;
mod instance;
mod intents;
mod mpris;
mod picker;
mod portal;
mod print;
mod reveal;
mod share;

/// The public path of the Linux desktop's implementations, `anyview_platform::linux`.
pub mod linux {
    pub use super::apps::DesktopApps;
    pub use super::instance::{BUS_NAME, DbusInstance, forward_over};
    pub use super::intents::{AGENT_BUS_NAME, OPEN_FILES};
    pub use super::mpris::{MPRIS_NAME, MprisSession};
    pub use super::picker::PortalPicker;
    pub use super::print::PortalPrinter;
    pub use super::reveal::FileManagerReveal;
    pub use super::share::MailShare;
    pub use crate::portable::{FreedesktopThumbnails, NoStacking, SystemOpen as XdgOpen};
}
