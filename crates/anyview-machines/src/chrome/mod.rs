//! The hover chrome: the titlebar and the OSD capsule that appear when the pointer moves and
//! hide after an idle delay, unless something pins them.

mod model;
mod pins;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Chrome, ChromeIn, ChromeOut, ChromeParams, Zone};
pub use pins::{PinReason, PinReasons};
