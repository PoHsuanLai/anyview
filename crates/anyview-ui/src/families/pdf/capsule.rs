//! The capsule's controls for a PDF: the page, the zoom, and the commands for fitting and finding.

use super::doc::{PdfDoc, room_of};
use super::live::page_view;
use super::scene::{Frame, fits, scale_of};
use crate::families::view::StageCx;
use crate::{Command, StageCommand};
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::prelude::Icon;

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
    vec![
        CapsuleSlot::button(
            stage(StageCommand::PreviousPage),
            "Previous page",
            Icon::ChevronUp,
        ),
        CapsuleSlot::Readout(page),
        CapsuleSlot::button(
            stage(StageCommand::NextPage),
            "Next page",
            Icon::ChevronDown,
        ),
        CapsuleSlot::Divider,
        CapsuleSlot::button(stage(StageCommand::ZoomOut), "Zoom out", Icon::Minus),
        CapsuleSlot::Readout(zoom),
        CapsuleSlot::button(stage(StageCommand::ZoomIn), "Zoom in", Icon::Plus),
        CapsuleSlot::Divider,
        CapsuleSlot::button(stage(StageCommand::ZoomToFit), "Fit page", Icon::Maximize),
        CapsuleSlot::button(stage(StageCommand::ZoomToWidth), "Fit width", Icon::Columns),
        CapsuleSlot::Divider,
        CapsuleSlot::button(stage(StageCommand::Find), "Find", Icon::Search),
    ]
}
