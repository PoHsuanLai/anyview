//! Fonts for the viewer: facts read with skrifa, a specimen set as vector outlines, and the font
//! peek.
//!
//! Blocking and effect-free except the one read of the file the peek is asked about: no runtime,
//! no spawning, no clock. WOFF is unpacked to the plain font it wraps; WOFF2 (Brotli and transformed
//! glyph tables) is named but not opened.
//!
//! Every public item is reached from this root, once.

mod error;
mod face;
mod peek;
mod specimen;
mod woff;

pub use error::FontError;
pub use face::{Face, Variation};
pub use peek::{FontPeek, FontPeeked};
pub use specimen::{EM, Specimen, SpecimenLine};
