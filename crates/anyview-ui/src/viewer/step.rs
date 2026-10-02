//! The root's transitions: route an input to its region, lift the outputs, and handle what
//! crosses regions.

use super::command::run;
use super::model::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use super::pins::{synced, wanted};
use super::region::{Step, chrome, panel, presentation, sheet, stage, stepped};
use crate::keys::{Regions, Route, route};
use crate::load::Ticket;
use crate::load::{LoadIn, LoadOut};
use crate::navigate::{NavigateIn, NavigateOut};
use crate::palette::{PaletteIn, PaletteOut};
use crate::presentation::Presentation;
use crate::stage::{Stage, StageFamily, StageIn};
use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};
use ds_core::machine::{Elapsed, Machine};
use ds_core::time::stamp::Stamp;
use ds_core::vocab::Shortcut;

impl Machine for Viewer {
    type In = ViewerIn;
    type Out = ViewerOut;
    type Params = ViewerParams;

    fn step(self, input: ViewerIn, at: Stamp, params: &ViewerParams) -> Step {
        let before = wanted(&self);
        let (viewer, mut outs) = apply(self, input, at, params);
        let (viewer, pinned) = synced(viewer, &before, at, params);
        outs.extend(pinned);
        (viewer, outs)
    }

    /// The earliest wake of any region; only the chrome keeps a timer, so a viewer at rest runs
    /// none.
    fn wake(&self) -> Option<Stamp> {
        [
            self.load.wake(),
            self.chrome.wake(),
            self.panel.wake(),
            self.palette.wake(),
            self.sheet.wake(),
            self.navigate.wake(),
            self.presentation.wake(),
            self.stage.wake(),
        ]
        .into_iter()
        .flatten()
        .min()
    }
}

fn apply(viewer: Viewer, input: ViewerIn, at: Stamp, params: &ViewerParams) -> Step {
    match input {
        ViewerIn::Open(path) => begin(viewer, &path, at, params),
        ViewerIn::Reload(path) => reload(viewer, &path, at),
        ViewerIn::Dropped(paths) => dropped(viewer, paths, at, params),
        ViewerIn::Load(input) => load(viewer, input, at, params),
        ViewerIn::Chrome(input) => chrome(viewer, input, at, params),
        ViewerIn::Panel(input) => panel(viewer, input, at, params),
        ViewerIn::Palette(input) => palette(viewer, input, at, params),
        ViewerIn::Sheet(input) => sheet(viewer, input, at, params),
        ViewerIn::Navigate(input) => navigate(viewer, input, at, params),
        ViewerIn::Presentation(input) => presentation(viewer, input, at, params),
        ViewerIn::StartAs(presentation) => (
            Viewer {
                presentation,
                ..viewer
            },
            vec![],
        ),
        ViewerIn::Stage(input) => stage(viewer, input, at, params),
        ViewerIn::Run(command) => run(viewer, command, at, params),
        ViewerIn::Key(key) => keyed(viewer, &key, at, params),
        ViewerIn::Elapsed => elapsed(viewer, at, params),
    }
}

/// Start loading `path`: the load takes a new ticket, and whatever stage showed the file before
/// goes, so nothing of it answers inputs meant for the next one.
fn begin(viewer: Viewer, path: &FilePath, at: Stamp, _params: &ViewerParams) -> Step {
    let (viewer, outs) = restart(viewer, path, at, |ticket, path| ViewerOut::Probe {
        ticket,
        path,
    });
    (
        Viewer {
            stage: Stage::NoStage,
            ..viewer
        },
        outs,
    )
}

/// Load `path` again after it changed: the stage stays, so the new copy opens where the old one
/// was left (the probe of the same family keeps it, see `load`).
fn reload(viewer: Viewer, path: &FilePath, at: Stamp) -> Step {
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
    let (load, outs) = viewer.load.step(LoadIn::Begin, at, &());
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

/// Files dropped on the window. The first opens and the walk is over until a list for it exists:
/// one file's list is its folder (asked of the window), many files are the list.
fn dropped(viewer: Viewer, paths: Vec<FilePath>, at: Stamp, params: &ViewerParams) -> Step {
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
fn load(viewer: Viewer, input: LoadIn, at: Stamp, params: &ViewerParams) -> Step {
    match input {
        LoadIn::Begin => (viewer, vec![]),
        LoadIn::Probed { .. }
        | LoadIn::Peeked { .. }
        | LoadIn::PeekFailed { .. }
        | LoadIn::Opened { .. }
        | LoadIn::Failed { .. }
        | LoadIn::Elapsed => {
            let (load, outs) = viewer.load.step(input, at, &());
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
            let outs = outs.into_iter().map(ViewerOut::Load).collect();
            (
                Viewer {
                    load,
                    stage,
                    ..viewer
                },
                outs,
            )
        }
    }
}

/// The stage for a file of `family`: the one showing when it is already of that family (a reload
/// keeps its place), otherwise a new one.
fn kept_or_new(showing: &Stage, family: StageFamily, params: &ViewerParams) -> Stage {
    if showing.family() == family {
        showing.clone()
    } else {
        Stage::for_family(family, params.stage.text.views)
    }
}

/// The palette's own transitions, then what it ran.
fn palette(viewer: Viewer, input: PaletteIn, at: Stamp, params: &ViewerParams) -> Step {
    let (palette, outs) = viewer.palette.clone().step(input, at, &params.palette);
    let viewer = Viewer { palette, ..viewer };
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

/// A move along the sequence; landing on a file begins loading it.
fn navigate(viewer: Viewer, input: NavigateIn, at: Stamp, params: &ViewerParams) -> Step {
    let (navigate, outs) = viewer.navigate.clone().step(input, at, &());
    let viewer = Viewer { navigate, ..viewer };
    outs.into_iter()
        .fold((viewer, vec![]), |(viewer, mut outs), out| match out {
            NavigateOut::Open(path) => {
                let (viewer, more) = begin(viewer, &path, at, params);
                outs.extend(more);
                (viewer, outs)
            }
            NavigateOut::Preload(neighbours) => {
                outs.push(ViewerOut::Preload(neighbours));
                (viewer, outs)
            }
        })
}

fn keyed(viewer: Viewer, key: &Shortcut, at: Stamp, params: &ViewerParams) -> Step {
    let routed = route(
        key,
        Regions {
            sheet: &viewer.sheet,
            palette: &viewer.palette,
            panel: &viewer.panel,
            stage: &viewer.stage,
            stage_params: &params.stage,
        },
    );
    match routed {
        Route::Sheet(input) => sheet(viewer, input, at, params),
        Route::Palette(input) => palette(viewer, input, at, params),
        Route::OpenPalette => palette(viewer, PaletteIn::Open, at, params),
        Route::Panel(input) => panel(viewer, input, at, params),
        Route::CloseWindow => (viewer, vec![ViewerOut::CloseWindow]),
        Route::OpenFile => (viewer, vec![ViewerOut::PickFile]),
        Route::Dismiss => dismissed(viewer),
        Route::Stage(input) => stage(viewer, input, at, params),
        Route::Navigate(input) => navigate(viewer, input, at, params),
        Route::Chrome(input) => chrome(viewer, input, at, params),
        Route::Swallowed | Route::Ignored => (viewer, vec![]),
    }
}

/// Esc with nothing open: a quick look closes; a window, a mini window and a background session
/// stay.
fn dismissed(viewer: Viewer) -> Step {
    match viewer.presentation {
        Presentation::Peek => (viewer, vec![ViewerOut::CloseWindow]),
        Presentation::Window | Presentation::Mini | Presentation::Background => (viewer, vec![]),
    }
}

/// The clock for every region.
fn elapsed(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let (viewer, mut outs) = chrome(viewer, Elapsed.into(), at, params);
    let (load, more) = stepped(viewer.load, Elapsed.into(), at, &(), ViewerOut::Load);
    outs.extend(more);
    let viewer = Viewer { load, ..viewer };
    let (viewer, more) = panel(viewer, Elapsed.into(), at, params);
    outs.extend(more);
    let (viewer, more) = palette(viewer, Elapsed.into(), at, params);
    outs.extend(more);
    let (viewer, more) = sheet(viewer, Elapsed.into(), at, params);
    outs.extend(more);
    let (viewer, more) = navigate(viewer, Elapsed.into(), at, params);
    outs.extend(more);
    let (viewer, more) = presentation(viewer, Elapsed.into(), at, params);
    outs.extend(more);
    let (viewer, more) = stage(viewer, StageIn::Elapsed, at, params);
    outs.extend(more);
    (viewer, outs)
}
