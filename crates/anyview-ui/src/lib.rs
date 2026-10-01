//! The viewer's pure state machines. Every region of the viewer (chrome, panel, palette, sheet,
//! navigation, presentation, loading, the four stages) is a [`ds_core::machine::Machine`]: it
//! takes an input and the time it happened, and returns its next state and the effects it wants
//! as data. Nothing here reads a clock, touches a file or draws; the views and the platform edge
//! above carry the effects out.
//!
//! Every public item is reached from this root, once.

mod chrome;
mod time;

#[cfg(test)]
mod testing;

pub use chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams, PinReason, PinReasons, Zone};
