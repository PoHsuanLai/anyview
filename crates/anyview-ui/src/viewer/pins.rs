//! The chrome's pins that follow from the other regions: a menu or sheet open, media paused.
//! The pointer and keyboard-focus pins come from the view directly.

use super::model::{Viewer, ViewerParams};
use super::region::{Step, chrome};
use crate::chrome::{ChromeIn, PinReason};
use crate::palette::Palette;
use crate::sheet::Sheet;
use crate::stage::{MediaStage, Stage};
use ds_core::time::stamp::Stamp;
use ds_core::word::Word;

/// Whether a reason to hold the chrome follows from the regions' states.
fn derived(viewer: &Viewer, reason: PinReason) -> bool {
    match reason {
        PinReason::MenuOpen => {
            let palette = match viewer.palette {
                Palette::Open { .. } => true,
                Palette::Closed => false,
            };
            let sheet = match viewer.sheet {
                Sheet::Export { .. }
                | Sheet::Unavailable { .. }
                | Sheet::ConfirmTrash
                | Sheet::Rename { .. }
                | Sheet::SaveCopy { .. }
                | Sheet::Revert { .. }
                | Sheet::NoVersions => true,
                Sheet::Closed => false,
            };
            palette || sheet
        }
        PinReason::MediaPaused => matches!(viewer.stage, Stage::Media(MediaStage::Paused { .. })),
        PinReason::PointerOverCapsule | PinReason::KeyboardFocus => false,
    }
}

/// The reasons the regions hold the chrome for right now.
pub(super) fn wanted(viewer: &Viewer) -> Vec<PinReason> {
    PinReason::ALL
        .iter()
        .copied()
        .filter(|reason| derived(viewer, *reason))
        .collect()
}

/// The chrome told about every reason that began or ended between `before` and now.
pub(super) fn synced(
    viewer: Viewer,
    before: &[PinReason],
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let now = wanted(&viewer);
    let began = now.iter().filter(|reason| !before.contains(reason));
    let ended = before.iter().filter(|reason| !now.contains(reason));
    let changes: Vec<ChromeIn> = began
        .map(|reason| ChromeIn::Pin(*reason))
        .chain(ended.map(|reason| ChromeIn::Unpin(*reason)))
        .collect();
    changes
        .into_iter()
        .fold((viewer, vec![]), |(viewer, mut outs), change| {
            let (viewer, more) = chrome(viewer, change, at, params);
            outs.extend(more);
            (viewer, outs)
        })
}
