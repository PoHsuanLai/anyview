//! Opening a file: beginning a load, reloading, files dropped or chosen, and the load's results.

use super::find::palette;
use super::model::{Choosing, PanelSay, Trashing, Viewer, ViewerOut, ViewerParams};
use super::region::{Step, sheet, stage};
use super::step::{context, navigate};
use crate::context::ContextIn;
use crate::hand::{HandIn, Tool};
use crate::load::{Load, LoadFailure, LoadIn, LoadOut, Ticket};
use crate::markup::PdfMarkup;
use crate::navigate::{Navigate, NavigateIn};
use crate::palette::PaletteIn;
use crate::panel::{Panel, PanelOut, PanelTab};
use crate::picture::PictureEdits;
use crate::sheet::SheetIn;
use crate::stage::{Stage, StageFamily};
use crate::trim::MediaTrim;
use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};
use ds_core::time::stamp::Stamp;

/// Start loading `path`: the load takes a new ticket, and whatever showed the file before goes,
/// so a different file opens as the first did and nothing of the last answers inputs meant for the
/// next one. The stage (its zoom, place, turn and find), the sheet, the palette and the context
/// menu are all about the file left. The side panel stays up, as a sidebar does in Preview.
pub(super) fn begin(viewer: Viewer, path: &FilePath, at: Stamp, params: &ViewerParams) -> Step {
    let (viewer, mut outs) = restart(viewer, path, at, |ticket, path| ViewerOut::Probe {
        ticket,
        path,
    });
    // The tool is the person's choice and carries to the next file, except the crop tool, which is
    // about the picture left, and the markup tool, about the pages left; a held Space and the
    // picture's place are about the file left too.
    let mut hand = viewer.hand.step(HandIn::SpaceUp);
    if hand.tool == Tool::Crop || hand.tool == Tool::Markup {
        hand.tool = Tool::Pan;
    }
    let viewer = Viewer {
        stage: Stage::NoStage,
        trashing: Trashing::Not,
        hand,
        picture: PictureEdits::default(),
        trim: MediaTrim::default(),
        markup: PdfMarkup::default(),
        after_save: None,
        ..viewer
    };
    let (viewer, more) = sheet(viewer, SheetIn::Cancel, at, params);
    outs.extend(more);
    let (viewer, more) = palette(viewer, PaletteIn::Close, at, params);
    outs.extend(more);
    let (viewer, more) = context(viewer, ContextIn::Close, at, params);
    outs.extend(more);
    (viewer, outs)
}

/// Load `path` again after it changed: the stage stays, so the new copy opens where the old one
/// was left (the probe of the same family keeps it, see `load`).
pub(super) fn reload(viewer: Viewer, path: &FilePath, at: Stamp) -> Step {
    restart(viewer, path, at, |ticket, path| ViewerOut::Reload {
        ticket,
        path,
    })
}

/// The load begun afresh: its outputs, with the probe made into the root's own `probe` output.
fn restart(
    viewer: Viewer,
    path: &FilePath,
    at: Stamp,
    probe: fn(Ticket, FilePath) -> ViewerOut,
) -> Step {
    let (load, outs) = viewer.load.step(LoadIn::Begin, at, &(), &());
    let outs = outs
        .into_iter()
        .map(|out| match out {
            LoadOut::Probe(ticket) => probe(ticket, path.clone()),
            LoadOut::Peek(_)
            | LoadOut::Open(_)
            | LoadOut::Cancel(_)
            | LoadOut::UseStage(_)
            | LoadOut::ShowFirstFrame(_)
            | LoadOut::ShowFull(_) => ViewerOut::Load(out),
        })
        .collect();
    (Viewer { load, ..viewer }, outs)
}

/// Ask for a file chooser, unless one is up already and has not answered.
pub(super) fn choose(viewer: Viewer) -> Step {
    match viewer.choosing {
        Choosing::Asked => (viewer, vec![]),
        Choosing::Not => (
            Viewer {
                choosing: Choosing::Asked,
                ..viewer
            },
            vec![ViewerOut::PickFile],
        ),
    }
}

/// Files dropped on the window, or chosen in a file chooser. The first opens and the walk is over until a list for it exists:
/// one file's list is its folder (asked of the window), many files are the list.
pub(super) fn dropped(
    viewer: Viewer,
    paths: Vec<FilePath>,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let Some(first) = paths.first().cloned() else {
        return (viewer, vec![]);
    };
    let (viewer, mut outs) = navigate(viewer, NavigateIn::Leave, at, params);
    let (viewer, more) = begin(viewer, &first, at, params);
    outs.extend(more);
    match NonEmpty::from_vec(paths) {
        Some(entries) if entries.count().get() > 1 => {
            let sequence = Sequence::new(entries, SequenceOrigin::Selection);
            let (viewer, more) = navigate(viewer, NavigateIn::Start(sequence), at, params);
            outs.extend(more);
            (viewer, outs)
        }
        Some(_) | None => {
            outs.push(ViewerOut::ListFolder(first));
            (viewer, outs)
        }
    }
}

/// A result of the load in flight. A stage the probe chose replaces the one showing.
pub(super) fn load(viewer: Viewer, input: LoadIn, at: Stamp, params: &ViewerParams) -> Step {
    match input {
        LoadIn::Begin => (viewer, vec![]),
        LoadIn::Probed { .. }
        | LoadIn::Peeked { .. }
        | LoadIn::PeekFailed { .. }
        | LoadIn::Opened { .. }
        | LoadIn::Failed { .. }
        | LoadIn::Elapsed => {
            let was_failed = matches!(viewer.load, Load::Failed { .. });
            let (load, outs) = viewer.load.step(input, at, &(), &());
            let vanished = !was_failed
                && matches!(
                    load,
                    Load::Failed {
                        reason: LoadFailure::NotFound,
                        ..
                    }
                );
            let stage = outs
                .iter()
                .find_map(|out| match out {
                    LoadOut::UseStage(family) => Some(*family),
                    LoadOut::Probe(_)
                    | LoadOut::Peek(_)
                    | LoadOut::Open(_)
                    | LoadOut::Cancel(_)
                    | LoadOut::ShowFirstFrame(_)
                    | LoadOut::ShowFull(_) => None,
                })
                .map_or(viewer.stage.clone(), |family| {
                    kept_or_new(&viewer.stage, family, params)
                });
            let family = outs.iter().find_map(|out| match out {
                LoadOut::UseStage(family) => Some(*family),
                LoadOut::Probe(_)
                | LoadOut::Peek(_)
                | LoadOut::Open(_)
                | LoadOut::Cancel(_)
                | LoadOut::ShowFirstFrame(_)
                | LoadOut::ShowFull(_) => None,
            });
            let mut outs: Vec<ViewerOut> = outs.into_iter().map(ViewerOut::Load).collect();
            let viewer = Viewer {
                load,
                stage,
                ..viewer
            };
            let (viewer, opened) = paged(viewer, family);
            outs.extend(opened);
            if vanished {
                gone(viewer, at, params)
            } else {
                (viewer, outs)
            }
        }
    }
}

/// A PDF or a book opens the side panel on its pages, as Preview does, until the person has said
/// what they want of the panel in this window. (The tab is set directly: the file's tabs are not
/// known until it lands, and the window shows the first tab the file has when this one is not.)
fn paged(viewer: Viewer, family: Option<StageFamily>) -> Step {
    let opens = family == Some(StageFamily::Pdf)
        && viewer.panel_said == PanelSay::Unsaid
        && viewer.panel == Panel::Hidden;
    if !opens {
        return (viewer, vec![]);
    }
    (
        Viewer {
            panel: Panel::Shown {
                tab: PanelTab::Thumbnails,
            },
            ..viewer
        },
        vec![ViewerOut::Panel(PanelOut::Show(PanelTab::Thumbnails))],
    )
}

/// The file on screen is not on disk. With others in the list the walk moves on and the file
/// leaves it, as a viewer does when a picture is deleted beside the one being looked at (and on
/// an arrow into a file that went, the arrow goes on). Trashed with nothing left to show, the
/// window closes; anything else leaves the failed screen, which says the file is gone.
fn gone(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let others = matches!(
        &viewer.navigate,
        Navigate::Walking { sequence, .. } if sequence.entries().count().get() > 1
    );
    match (others, viewer.trashing) {
        (true, _) => navigate(viewer, NavigateIn::Gone, at, params),
        (false, Trashing::Underway) => (
            Viewer {
                trashing: Trashing::Not,
                ..viewer
            },
            vec![ViewerOut::CloseWindow],
        ),
        (false, Trashing::Not) => (viewer, vec![]),
    }
}

/// The stage for a file of `family`: the one showing when it is already of that family (a reload
/// keeps its place), otherwise a new one.
fn kept_or_new(showing: &Stage, family: StageFamily, params: &ViewerParams) -> Stage {
    if showing.family() == family {
        showing.clone()
    } else {
        Stage::for_family(family, params.stage.text.views).wrapped(params.stage.text.wrap)
    }
}
