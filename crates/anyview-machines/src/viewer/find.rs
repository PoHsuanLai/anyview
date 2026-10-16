//! The palette's transitions and the find the palette stands for.

use super::command::run;
use super::model::{Viewer, ViewerOut, ViewerParams};
use super::region::{Step, stage};
use crate::command::Command;
use crate::palette::{Palette, PaletteIn, PaletteIndex, PaletteOut, PaletteScope, step_with_scope};
use crate::presentation::Presentation;
use crate::typed::TypedText;
use ds_core::time::stamp::Stamp;

/// The palette's own transitions, then what it ran, then the find the palette stands for.
pub(super) fn palette(viewer: Viewer, input: PaletteIn, at: Stamp, params: &ViewerParams) -> Step {
    // A pane draws no palette (its commands are listed in the host's), so it never opens one.
    if viewer.presentation == Presentation::Pane {
        return (viewer, vec![]);
    }
    let before = (viewer.palette.clone(), viewer.palette_scope);
    let closing = matches!(input, PaletteIn::Close);
    let (viewer, mut outs) = palette_stepped(viewer, input, at, params);
    let (viewer, more) = find_synced(viewer, &before, closing, at, params);
    outs.extend(more);
    (viewer, outs)
}

fn palette_stepped(viewer: Viewer, input: PaletteIn, at: Stamp, params: &ViewerParams) -> Step {
    let (palette, palette_scope, outs) = step_with_scope(
        viewer.palette.clone(),
        viewer.palette_scope,
        input,
        at,
        &params.palette,
    );
    let viewer = Viewer {
        palette,
        palette_scope,
        ..viewer
    };
    outs.into_iter()
        .fold((viewer, vec![]), |(viewer, mut outs), out| match out {
            PaletteOut::Run(command) => {
                let (viewer, more) = run(viewer, command, at, params);
                outs.extend(more);
                (viewer, outs)
            }
            PaletteOut::Opened | PaletteOut::Closed => {
                outs.push(ViewerOut::Palette(out));
                (viewer, outs)
            }
        })
}

/// ⌘F, or the Find button: the palette opens as a find, on the text of the find that is up (so its
/// last text is there to replace), else on none.
pub(super) fn open_find(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let last = viewer
        .stage
        .find_state()
        .map_or(TypedText::EMPTY, |(query, _)| query.clone());
    palette(viewer, PaletteIn::OpenFind(last), at, params)
}

/// The stage's find follows the palette while it is a find: what is typed is searched for, the row
/// the highlight moves to is the current hit (so the file behind shows it), and Esc, which closes
/// the palette without picking, puts the find away. A hit picked with Enter leaves the find up,
/// its marks on the file and ⌘G to step.
fn find_synced(
    viewer: Viewer,
    before: &(Palette, PaletteScope),
    closing: bool,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let (was, was_scope) = before;
    let finding_before = matches!(was_scope, PaletteScope::Find(_));
    let finding_now = matches!(viewer.palette_scope, PaletteScope::Find(_));
    match (was, viewer.palette.clone()) {
        (Palette::Open { .. }, Palette::Closed)
            if finding_before && closing && viewer.stage.is_finding() =>
        {
            match viewer.stage.dismissal() {
                Some(input) => stage(viewer, input, at, params),
                None => (viewer, vec![]),
            }
        }
        (
            Palette::Open {
                query: was_query,
                selection: row,
                ..
            },
            Palette::Open {
                query, selection, ..
            },
        ) if finding_now && finding_before && *was_query == query => {
            // The same text: the highlight may have moved to another hit.
            if *row == selection {
                (viewer, vec![])
            } else {
                jumped(viewer, selection, at, params)
            }
        }
        (Palette::Open { .. } | Palette::Closed, Palette::Open { query, .. }) if finding_now => {
            searched(viewer, &query, at, params)
        }
        (Palette::Open { .. } | Palette::Closed, Palette::Open { .. } | Palette::Closed) => {
            (viewer, vec![])
        }
    }
}

/// The find asked for `query`: nothing is searched until something is typed, so opening the
/// palette as a find does not turn a Markdown page into its source.
pub(super) fn searched(
    viewer: Viewer,
    query: &TypedText,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    if query.is_empty() && !viewer.stage.is_finding() {
        return (viewer, vec![]);
    }
    match viewer.stage.find_input(query) {
        Some(input) => stage(viewer, input, at, params),
        None => (viewer, vec![]),
    }
}

/// The highlight moved to `row`: when that row is a hit, it is the current one.
fn jumped(viewer: Viewer, row: PaletteIndex, at: Stamp, params: &ViewerParams) -> Step {
    match params.palette.rows.get(row.0) {
        Some(Command::FindHit(hit)) => match viewer.stage.hit_input(*hit) {
            Some(input) => stage(viewer, input, at, params),
            None => (viewer, vec![]),
        },
        Some(_) | None => (viewer, vec![]),
    }
}
