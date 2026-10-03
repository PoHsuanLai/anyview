//! Fonts for the viewer: facts read with skrifa, a specimen set as vector outlines, and the font
//! peek.
//!
//! Blocking and effect-free except the one read of the file the peek is asked about: no runtime,
//! no spawning, no clock. WOFF and WOFF2 are named but not opened.
//!
//! Every public item is reached from this root, once.

mod error;
mod face;
mod peek;
mod specimen;

pub use error::FontError;
pub use face::{Face, Variation};
pub use peek::{FontPeek, FontPeeked};
pub use specimen::{EM, Specimen, SpecimenLine};
