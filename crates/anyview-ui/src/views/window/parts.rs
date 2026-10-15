//! What the window works out from the machine's state and the shelf each time it draws: the tabs
//! the info panel offers, where the load stands, the name the window shows, the capsule's slots
//! and the hits of a find.

use super::super::session::{Probe, WHOLE_HITS, offered_slots, without_find};
use super::super::shelf::Shelf;
use crate::families::LoadedDoc;
use crate::io::HostRequest;
use crate::{
    Command, HitLine, Load, LoadFailure, Palette as PaletteState, PaletteScope, PaneChrome,
    PanelTabs, Stage, StageCx, Viewer,
};
use anyview_core::{FileAction, FilePath};
use dioxus::prelude::*;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::prelude::*;

/// The tabs the info panel offers for `current`, the document on screen: its own, with the
/// recording's according to what the player can do. A pane draws the panel only when its host
/// asked for it (`pane_chrome`), so with `hosted` and anything less it has none.
pub(super) fn panel_tabs(
    current: Option<&LoadedDoc>,
    state: &Viewer,
    shelf: Shelf,
    hosted: bool,
    pane_chrome: PaneChrome,
) -> PanelTabs {
    let tabs = current.map_or_else(Default::default, |doc| {
        let tabs = doc.view().panel_params().tabs;
        if matches!(state.stage, Stage::Media(_)) {
            crate::families::media_tabs(tabs, shelf.media.read().abilities)
        } else {
            tabs
        }
    });
    // A pane draws the info panel only when its host asked for it.
    if hosted && pane_chrome != PaneChrome::WithPanel {
        PanelTabs::default()
    } else {
        tabs
    }
}

/// Why the load failed, if it did, and what the content area shows meanwhile: the document once
/// there is one (`shown`), otherwise a loading screen for the operation under way.
pub(super) fn load_state(load: Load, shown: bool, shelf: Shelf) -> (Option<LoadFailure>, Phase) {
    let failure = match load {
        Load::Failed { reason, .. } => Some(reason),
        Load::Idle { .. }
        | Load::Probing { .. }
        | Load::Peeking { .. }
        | Load::Opening { .. }
        | Load::Ready { .. } => None,
    };
    let phase = match load {
        Load::Failed { .. } | Load::Ready { .. } => Phase::Ready,
        Load::Idle { .. } | Load::Probing { .. } | Load::Peeking { .. } | Load::Opening { .. } => {
            if shown {
                Phase::Ready
            } else {
                Phase::Loading((shelf.operation)())
            }
        }
    };
    (failure, phase)
}

/// The name the window shows: the probed file's, or the file it asked for while it probes.
pub(super) fn window_title(shelf: Shelf) -> String {
    title_of(&shelf.probe.read()).unwrap_or_else(|| {
        (shelf.wanted)()
            .as_ref()
            .and_then(FilePath::file_name)
            .map_or_else(String::new, |name| name.as_str().to_owned())
    })
}

/// The lines the palette lists under "In This File" when it is a find, and how many hits there
/// are. A pane lists them for its host's palette while a find is up (`hosted`).
pub(super) fn find_hits(
    state: &Viewer,
    current: Option<&LoadedDoc>,
    cx: &StageCx,
    hosted: bool,
) -> (Vec<HitLine>, u32) {
    let listing_hits = (hosted && state.stage.is_finding())
        || matches!(
            (&state.palette, state.palette_scope),
            (PaletteState::Open { .. }, PaletteScope::Find(_))
        );
    let hit_lines = match current {
        Some(doc) if listing_hits => doc.view().hit_lines(cx, WHOLE_HITS),
        Some(_) | None => Vec::new(),
    };
    let found = state
        .stage
        .find_state()
        .and_then(|(_, hits)| hits.count())
        .map_or(0, |count| count.0);
    (hit_lines, found)
}

/// The file's name, for a question about it.
pub(super) fn title_of(probe: &Probe) -> Option<String> {
    probe
        .found()
        .and_then(|probed| probed.source.path().file_name())
        .map(|name| name.as_str().to_owned())
}

/// Show the file the window last asked for in its folder.
pub(super) fn reveal(edge: &crate::Edge, shelf: &Shelf) {
    if let Some(file) = shelf.wanted.peek().clone() {
        edge.request(HostRequest::Reveal(file));
    }
}

/// The capsule's slots for `current`, the document on screen: those the file takes (`files` is
/// what the palette and the menu list), so a button for a file action the file does not take (a
/// rotate on a file that refuses a save) is not drawn. A pane has no palette for the Find button
/// to open.
pub(super) fn capsule_slots(
    current: Option<&LoadedDoc>,
    cx: &StageCx,
    hosted: bool,
    files: &[FileAction],
) -> Vec<RankedSlot<Command>> {
    current
        .map(|doc| {
            let listed = doc.view().slots(cx);
            let listed = if hosted { without_find(listed) } else { listed };
            offered_slots(listed, files)
        })
        .unwrap_or_default()
}
