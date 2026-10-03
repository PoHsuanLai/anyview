//! Stepping one region of the viewer and lifting what it wants into the root's outputs.

use super::model::{Viewer, ViewerOut, ViewerParams};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

pub(super) type Step = (Viewer, Vec<ViewerOut>);

/// A region stepped on `input`: its next state and its outputs, lifted.
pub(super) fn stepped<M: Machine>(
    machine: M,
    input: M::In,
    at: Stamp,
    params: &M::Params,
    lift: fn(M::Out) -> ViewerOut,
) -> (M, Vec<ViewerOut>) {
    let (next, outs) = machine.step(input, at, params);
    (next, outs.into_iter().map(lift).collect())
}

/// The root after `viewer` with its region replaced, and the outputs that came with it.
pub(super) fn chrome(
    viewer: Viewer,
    input: crate::chrome::ChromeIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let (chrome, outs) = stepped(viewer.chrome, input, at, &params.chrome, ViewerOut::Chrome);
    (Viewer { chrome, ..viewer }, outs)
}

pub(super) fn panel(
    viewer: Viewer,
    input: crate::panel::PanelIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let (panel, outs) = stepped(viewer.panel, input, at, &params.panel, ViewerOut::Panel);
    (Viewer { panel, ..viewer }, outs)
}

pub(super) fn sheet(
    viewer: Viewer,
    input: crate::sheet::SheetIn,
    at: Stamp,
    _params: &ViewerParams,
) -> Step {
    let (sheet, outs) = stepped(viewer.sheet, input, at, &(), ViewerOut::Sheet);
    (Viewer { sheet, ..viewer }, outs)
}

pub(super) fn presentation(
    viewer: Viewer,
    input: crate::presentation::PresentationIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let (presentation, outs) = stepped(
        viewer.presentation,
        input,
        at,
        &params.presentation,
        ViewerOut::Presentation,
    );
    (
        Viewer {
            presentation,
            ..viewer
        },
        outs,
    )
}

pub(super) fn stage(
    viewer: Viewer,
    input: crate::stage::StageIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let (stage, outs) = stepped(viewer.stage, input, at, &params.stage, ViewerOut::Stage);
    (Viewer { stage, ..viewer }, outs)
}
