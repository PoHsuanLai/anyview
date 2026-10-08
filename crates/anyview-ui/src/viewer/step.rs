//! The root's transitions: route an input to its region, lift the outputs, and handle what
//! crosses regions.

use super::command::run;
use super::model::{Choosing, Trashing, Viewer, ViewerIn, ViewerOut, ViewerParams};
use super::pins::{synced, wanted};
use super::region::{Step, chrome, panel, presentation, sheet, stage, stepped};
use crate::command::Command;
use crate::context::{ContextIn, ContextOut, ContextPick};
use crate::keys::{Regions, Route, route};
use crate::load::Ticket;
use crate::load::{Load, LoadFailure, LoadIn, LoadOut};
use crate::navigate::{Navigate, NavigateIn, NavigateOut};
use crate::palette::{Palette, PaletteIn, PaletteOut};
use crate::panel::{PanelIn, PanelTab};
use crate::presentation::Presentation;
use crate::sheet::{Sheet, SheetIn, SheetOut};
use crate::stage::{Stage, StageFamily, StageIn};
use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin, shortcut};
use ds_core::machine::{Elapsed, Machine};
use ds_core::time::stamp::Stamp;
use ds_core::vocab::Shortcut;

impl Machine for Viewer {
    type In = ViewerIn;
    type Out = ViewerOut;
    /// Nothing is set: everything the machine reads from outside is `Ctx`, read at each step.
    type Params = ();
    type Ctx = ViewerParams;

    fn step(self, input: ViewerIn, at: Stamp, _: &(), params: &ViewerParams) -> Step {
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
            self.context.wake(),
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
        ViewerIn::Chosen(paths) => {
            let viewer = Viewer {
                choosing: Choosing::Not,
                ..viewer
            };
            dropped(viewer, paths, at, params)
        }
        ViewerIn::Load(input) => load(viewer, input, at, params),
        ViewerIn::Chrome(input) => chrome(viewer, input, at, params),
        ViewerIn::Panel(input) => panel(viewer, input, at, params),
        ViewerIn::Palette(input) => palette(viewer, input, at, params),
        ViewerIn::Context(input) => context(viewer, input, at, params),
        ViewerIn::Sheet(input) => sheet_in(viewer, input, at, params),
        ViewerIn::Navigate(input) => navigate(viewer, input, at, params),
        ViewerIn::Presentation(input) => presentation(viewer, input, at, params),
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
            trashing: Trashing::Not,
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
fn dropped(viewer: Viewer, paths: Vec<FilePath>, at: Stamp, params: &ViewerParams) -> Step {
    let Some(first) = paths.first().cloned() else {
        return (viewer, vec![]);
    };
    // A sheet is about the file it was opened on: the new file is not what it would apply to.
    let (viewer, mut outs) = sheet(viewer, SheetIn::Cancel, at, params);
    let (viewer, more) = navigate(viewer, NavigateIn::Leave, at, params);
    outs.extend(more);
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
            let outs = outs.into_iter().map(ViewerOut::Load).collect();
            let viewer = Viewer {
                load,
                stage,
                ..viewer
            };
            if vanished {
                gone(viewer, at, params)
            } else {
                (viewer, outs)
            }
        }
    }
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

/// An input of the sheet; confirming Move to Trash marks the file as on its way out.
fn sheet_in(
    viewer: Viewer,
    input: crate::sheet::SheetIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let (viewer, outs) = sheet(viewer, input, at, params);
    let trashed = outs
        .iter()
        .any(|out| matches!(out, ViewerOut::Sheet(SheetOut::Trash)));
    let trashing = if trashed {
        Trashing::Underway
    } else {
        viewer.trashing
    };
    (Viewer { trashing, ..viewer }, outs)
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
    let (palette, outs) = viewer.palette.clone().step(input, at, &params.palette, &());
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

/// The context menu's own transitions, then what the row picked did. It opens over a file that is
/// showing and nothing else: a sheet or the palette has the person's attention already.
fn context(viewer: Viewer, input: ContextIn, at: Stamp, params: &ViewerParams) -> Step {
    let opening = matches!(input, ContextIn::Open(_) | ContextIn::OpenAtCentre);
    if opening && !can_open_context(&viewer) {
        return (viewer, vec![]);
    }
    let (context, outs) = viewer.context.step(input, at, &params.context, &());
    let viewer = Viewer { context, ..viewer };
    outs.into_iter()
        .fold((viewer, vec![]), |(viewer, mut outs), out| {
            let (viewer, more) = match out {
                ContextOut::Run(ContextPick::Run(command)) => run(viewer, command, at, params),
                ContextOut::Run(ContextPick::GetInfo) => {
                    panel(viewer, PanelIn::Choose(PanelTab::Info), at, params)
                }
            };
            outs.extend(more);
            (viewer, outs)
        })
}

/// Whether a context menu may open now: a file is on screen, and no sheet or palette is up.
fn can_open_context(viewer: &Viewer) -> bool {
    let showing = matches!(viewer.load, Load::Ready { .. });
    let free = matches!(viewer.sheet, Sheet::Closed) && matches!(viewer.palette, Palette::Closed);
    showing && free
}

/// A move along the sequence; landing on a file begins loading it.
fn navigate(viewer: Viewer, input: NavigateIn, at: Stamp, params: &ViewerParams) -> Step {
    let (navigate, outs) = viewer.navigate.clone().step(input, at, &(), &());
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
            context: &viewer.context,
            panel: &viewer.panel,
            stage: &viewer.stage,
            stage_params: &params.stage,
            pick_files: params.platform.pick_files,
        },
    );
    match routed {
        Route::Sheet(input) => sheet_in(viewer, input, at, params),
        Route::Palette(input) => palette(viewer, input, at, params),
        Route::OpenPalette => palette(viewer, PaletteIn::Open, at, params),
        Route::Context(input) => context(viewer, input, at, params),
        Route::OpenContextMenu => context(viewer, ContextIn::OpenAtCentre, at, params),
        Route::Panel(input) => panel(viewer, input, at, params),
        Route::CloseWindow => (viewer, vec![ViewerOut::CloseWindow]),
        Route::OpenFile => choose(viewer),
        Route::Rewind(rewind) => (viewer, vec![ViewerOut::Rewind(rewind)]),
        Route::Dismiss => dismissed(viewer),
        Route::Stage(input) => stage(viewer, input, at, params),
        Route::Navigate(input) => navigate(viewer, input, at, params),
        Route::Chrome(input) => chrome(viewer, input, at, params),
        Route::Ignored => file_key(viewer, key, at, params),
        Route::Swallowed => (viewer, vec![]),
    }
}

/// A key no region claimed, which may be the shortcut of a file action the open file offers (⌘P,
/// ⌘D, ⌘[ …): the action runs as the palette's row would.
fn file_key(viewer: Viewer, key: &Shortcut, at: Stamp, params: &ViewerParams) -> Step {
    let keys = key.keys();
    let action = params
        .files
        .iter()
        .find(|action| shortcut(**action).is_some_and(|bound| bound.keys() == keys))
        .copied();
    match action {
        Some(action) => run(viewer, Command::File(action), at, params),
        None => (viewer, vec![]),
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
    let (viewer, more) = context(viewer, Elapsed.into(), at, params);
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
