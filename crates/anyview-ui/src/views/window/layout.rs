//! The window's layout rules: how wide the left panel may be dragged, when it lies over the
//! content, how much of the titlebar the mode control takes, and whether the capsule draws at all.

use super::super::chrome::BarRoom;
use crate::Command;
use crate::families::Area;
use ds::components::chrome::capsule::fit::{SlotShown, fit};
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::components::chrome::split_view::model::{Collapsing, PaneSize, PaneSpec};
use ds::prelude::*;

/// The least the left panel is dragged to, in logical pixels.
const PANEL_LEAST: u32 = 220;

/// The most the left panel is dragged to, in logical pixels.
const PANEL_MOST: u32 = 320;

/// The left panel's pane: 240 wide by default, or the width the person left it at, between 220 and
/// 320, and it folds away when dragged past half its least. It sits beside the stage, so opening
/// it shrinks the stage and not the window. A width kept from another launch is held to that
/// range, so a stale or edited value cannot give a pane the drag would not.
pub(super) fn panel_pane(kept: Option<u32>) -> PaneSpec {
    let width = kept.map_or(super::super::panel::WIDTH, |kept| {
        kept.clamp(PANEL_LEAST, PANEL_MOST)
    });
    PaneSpec {
        preferred: PaneSize::Fixed(Px(width as f32)),
        min: PaneSize::Fixed(Px(PANEL_LEAST as f32)),
        max: PaneSize::Fixed(Px(PANEL_MOST as f32)),
        collapsing: Collapsing::Snaps,
    }
}

/// The width of the left panel in `sizes`, the sizes a split view reports once a drag settles, in
/// whole logical pixels.
pub(super) fn panel_width_in(sizes: &[PaneSize]) -> Option<u32> {
    match sizes.first()? {
        PaneSize::Fixed(Px(width)) => Some(width.round().max(0.0) as u32),
        PaneSize::Share(_) => None,
    }
}

/// Below this width the info panel lies over the content instead of beside it (a pane or a window
/// alike): opening it would leave the content too little room.
pub(super) const NARROW_BELOW: f32 = 480.0;

/// Below this width the titlebar's mode control (Select, Pan, Crop) takes the right of the bar, so
/// the title ends before it instead of running under it.
const BAR_TIGHT_BELOW: f32 = 560.0;

/// Below this width the bar has no room for the mode control beside the lights and a title, so the
/// control steps aside (its commands stay in the palette and the menu).
const BAR_BARE_BELOW: f32 = 300.0;

/// Whether the capsule would draw anything on a stage `area` wide: quire thins a capsule by rank
/// and an unlucky width leaves none, which would draw as an empty pill.
pub(super) fn capsule_shows_something(slots: &[RankedSlot<Command>], area: Option<Area>) -> bool {
    let width = area
        .map(|area| area.size.width.0.floor().max(0.0) as u32)
        .filter(|width| *width > 0);
    fit(slots, width).contains(&SlotShown::Shown)
}

/// How much of the titlebar the mode control takes, by the window's width in logical pixels.
pub(super) fn bar_room(width: Option<f32>) -> BarRoom {
    match width {
        Some(width) if width < BAR_BARE_BELOW => BarRoom::Bare,
        Some(width) if width < BAR_TIGHT_BELOW => BarRoom::Tight,
        Some(_) | None => BarRoom::Roomy,
    }
}
