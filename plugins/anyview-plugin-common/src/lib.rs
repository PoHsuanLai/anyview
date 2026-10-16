//! What the plugins that run the person's own programs (anyview-ffmpeg, and anyview-heif and
//! anyview-raw through anyview-tool-kit) share: the size a picture is fitted to and the way a
//! program is found. It has no dependency, so a plugin takes it without taking `image` or `serde`.

#![warn(missing_docs)]

mod fit;
mod lookup;

pub use fit::{Fit, Picture, fitted};
pub use lookup::{Lookup, NotRunnable, find_program};
