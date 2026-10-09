//! What every platform has: no D-Bus and no freedesktop service is called from here. Single
//! instance over the per-user socket (`LatchkeyInstance`), the platform's own opener for links
//! and for showing a file in its folder, the shared thumbnail cache (a file layout), window
//! stacking as a normal window, and the abilities a platform without the desktop's services
//! lacks, as traits that say so. `linux` (feature `quire-desktop`) replaces some of these with
//! the desktop's own, and re-exports the rest.

mod absent;
mod frame;
mod instance;
mod open;
mod stacking;
mod thumbnails;

pub use absent::{NoPicker, NoPrinter, NoShare};
pub use instance::LatchkeyInstance;
pub use open::{SystemOpen, SystemReveal};
pub use stacking::NoStacking;
pub use thumbnails::FreedesktopThumbnails;
