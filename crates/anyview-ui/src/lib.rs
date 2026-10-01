//! The viewer's pure state machines. Every region of the viewer (chrome, panel, palette, sheet,
//! navigation, presentation, loading, the four stages) is a [`ds_core::machine::Machine`]: it
//! takes an input and the time it happened, and returns its next state and the effects it wants
//! as data. Nothing here reads a clock, touches a file or draws; the views and the platform edge
//! above carry the effects out.
//!
//! Every public item is reached from this root, once.

mod chrome;
mod command;
mod palette;
mod panel;
mod time;
mod typed;

#[cfg(test)]
mod testing;

pub use chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams, PinReason, PinReasons, Zone};
pub use command::{Command, StageCommand};
pub use palette::{Palette, PaletteIn, PaletteMove, PaletteOut, PaletteParams, RowIndex};
pub use panel::{Panel, PanelIn, PanelOut, PanelParams, PanelTab, PanelTabs};
pub use typed::TypedText;
