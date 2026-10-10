//! The side panel: hidden by default, one tab at a time, and only the tabs the format has.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{Panel, PanelIn, PanelOut, PanelParams, PanelTab, PanelTabs};
