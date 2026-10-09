//! The capsule's controls for a PDF: the page, the zoom, and the commands for fitting and finding.

use super::doc::{PdfDoc, room_of};
use super::live::page_view;
use super::scene::{Frame, fits, scale_of};
use crate::families::view::StageCx;
use crate::{Command, StageCommand};
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::prelude::Icon;

/// How soon each control goes when the capsule is too wide for the stage (quire's capsule).
const RANK_FIND: u8 = 3;
const RANK_FIT: u8 = 2;
const RANK_ZOOM: u8 = 1;

/// The controls, left to right: the page and its neighbours, the zoom, the fits and the find.
pub(super) fn slots(doc: &PdfDoc, cx: &StageCx) -> Vec<RankedSlot<Command>> {
    let Some(view) = page_view(&cx.stage) else {
        return Vec::new();
    };
    let scale = room_of(cx.area).map(|area| {
        let fit = fits(doc.sizes(), Frame::of(area));
        scale_of(view.zoom, fit)
    });
    let zoom = scale.map_or_else(String::new, |scale| format!("{}%", scale.0 / 10));
    let page = format!("{} of {}", view.page.0 + 1, doc.pages().get());
    let stage = |command| Command::Stage(command);
    // Soonest to go first: the fits and the find (the palette and the menu keep them), then the
    // zoom. The page and its neighbours stay.
    let mut slots = vec![
        CapsuleSlot::button(
            stage(StageCommand::PreviousPage),
            "Previous Page",
            Icon::ChevronUp,
        )
        .essential(),
        CapsuleSlot::Readout(page).essential(),
        CapsuleSlot::button(
            stage(StageCommand::NextPage),
            "Next Page",
            Icon::ChevronDown,
        )
        .essential(),
        CapsuleSlot::Divider.essential(),
        CapsuleSlot::button(stage(StageCommand::ZoomOut), "Zoom Out", Icon::Minus)
            .droppable(RANK_ZOOM),
        CapsuleSlot::Readout(zoom).droppable(RANK_ZOOM),
        CapsuleSlot::button(stage(StageCommand::ZoomIn), "Zoom In", Icon::Plus)
            .droppable(RANK_ZOOM),
        CapsuleSlot::Divider.essential(),
        CapsuleSlot::button(stage(StageCommand::ZoomToFit), "Fit Page", Icon::Maximize)
            .droppable(RANK_FIT),
        CapsuleSlot::button(stage(StageCommand::ZoomToWidth), "Fit Width", Icon::Columns)
            .droppable(RANK_FIT),
        CapsuleSlot::Divider.essential(),
        CapsuleSlot::button(stage(StageCommand::Find), "Find", Icon::Search).droppable(RANK_FIND),
    ];
    slots.extend(crate::families::found::standing(&cx.stage));
    slots
}
