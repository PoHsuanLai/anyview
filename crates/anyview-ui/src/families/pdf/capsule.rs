//! The capsule's controls for a PDF: the page, the zoom, and the commands for fitting and finding.

use super::doc::{PdfDoc, room_of};
use super::live::page_view;
use super::scene::{Frame, fits, scale_of};
use crate::families::capsule_fit::{Ranked, fit_slots, stage_width};
use crate::families::view::StageCx;
use crate::{Command, StageCommand};
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::prelude::Icon;

/// How soon each control goes when the capsule is too wide for the stage (`capsule_fit`).
const RANK_FIND: u8 = 3;
const RANK_FIT: u8 = 2;
const RANK_ZOOM: u8 = 1;

/// The controls, left to right: the page and its neighbours, the zoom, the fits and the find.
pub(super) fn slots(doc: &PdfDoc, cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
    let Some(view) = page_view(&cx.stage) else {
        return Vec::new();
    };
    let scale = room_of(cx.area).map(|area| {
        let fit = fits(doc.sizes(), Frame::of(area));
        scale_of(view.zoom, fit)
    });
    let zoom = scale.map_or_else(String::new, |scale| format!("{}%", scale.0 / 10));
    let page = format!("{} / {}", view.page.0 + 1, doc.pages().get());
    let stage = |command| Command::Stage(command);
    // Soonest to go first: the fits and the find (the palette and the menu keep them), then the
    // zoom. The page and its neighbours stay.
    let slots = vec![
        Ranked::stays(CapsuleSlot::button(
            stage(StageCommand::PreviousPage),
            "Previous page",
            Icon::ChevronUp,
        )),
        Ranked::stays(CapsuleSlot::Readout(page)),
        Ranked::stays(CapsuleSlot::button(
            stage(StageCommand::NextPage),
            "Next page",
            Icon::ChevronDown,
        )),
        Ranked::stays(CapsuleSlot::Divider),
        Ranked::drops(
            RANK_ZOOM,
            CapsuleSlot::button(stage(StageCommand::ZoomOut), "Zoom out", Icon::Minus),
        ),
        Ranked::drops(RANK_ZOOM, CapsuleSlot::Readout(zoom)),
        Ranked::drops(
            RANK_ZOOM,
            CapsuleSlot::button(stage(StageCommand::ZoomIn), "Zoom in", Icon::Plus),
        ),
        Ranked::stays(CapsuleSlot::Divider),
        Ranked::drops(
            RANK_FIT,
            CapsuleSlot::button(stage(StageCommand::ZoomToFit), "Fit page", Icon::Maximize),
        ),
        Ranked::drops(
            RANK_FIT,
            CapsuleSlot::button(stage(StageCommand::ZoomToWidth), "Fit width", Icon::Columns),
        ),
        Ranked::stays(CapsuleSlot::Divider),
        Ranked::drops(
            RANK_FIND,
            CapsuleSlot::button(stage(StageCommand::Find), "Find", Icon::Search),
        ),
    ];
    fit_slots(slots, stage_width(cx.area))
}
