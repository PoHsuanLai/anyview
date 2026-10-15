//! The root's transitions: route an input to its region, lift the outputs, and handle what
//! crosses regions.

use super::command::run;
use super::departure::{leaving, saved, walks};
use super::find::palette;
use super::key::keyed;
use super::markup::{markup_first, markup_in, markup_synced};
use super::model::{Choosing, Trashing, Viewer, ViewerIn, ViewerOut, ViewerParams};
use super::opening::{begin, load, reload};
use super::picture::{answered, crop_synced, picture_in};
use super::pins::{synced, wanted};
use super::region::{Step, chrome, panel, presentation, sheet, stage, stepped};
use crate::context::{ContextIn, ContextOut, ContextPick};
use crate::hand::{HandIn, Tool};
use crate::load::Load;
use crate::navigate::{NavigateIn, NavigateOut};
use crate::palette::Palette;
use crate::panel::{PanelIn, PanelTab};
use crate::sheet::{Departure, Sheet, SheetIn, SheetOut};
use crate::stage::{Stage, StageIn};
use ds_core::machine::{Elapsed, Machine};
use ds_core::time::stamp::Stamp;

impl Machine for Viewer {
    type In = ViewerIn;
    type Out = ViewerOut;
    /// Nothing is set: everything the machine reads from outside is `Ctx`, read at each step.
    type Params = ();
    type Ctx = ViewerParams;

    fn step(self, input: ViewerIn, at: Stamp, _: &(), params: &ViewerParams) -> Step {
        let before = wanted(&self);
        let (viewer, outs) = apply(self, input, at, params);
        let mut outs = markup_first(&viewer, outs);
        let (viewer, pinned) = synced(viewer, &before, at, params);
        outs.extend(pinned);
        (markup_synced(crop_synced(viewer)), outs)
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
        ViewerIn::Open(path) => leaving(viewer, Departure::Open(path), at, params),
        ViewerIn::Reload(path) => reload(viewer, &path, at),
        ViewerIn::Dropped(paths) if paths.is_empty() => (viewer, vec![]),
        ViewerIn::Dropped(paths) => leaving(viewer, Departure::Dropped(paths), at, params),
        ViewerIn::Chosen(paths) => {
            let viewer = Viewer {
                choosing: Choosing::Not,
                ..viewer
            };
            if paths.is_empty() {
                (viewer, vec![])
            } else {
                leaving(viewer, Departure::Chosen(paths), at, params)
            }
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
        ViewerIn::Hand(input) => (hand(viewer, input), vec![]),
        ViewerIn::Picture(input) => picture_in(viewer, input, at, params),
        ViewerIn::Trim(input) => super::trim::trim_in(viewer, input, at, params),
        ViewerIn::Markup(input) => markup_in(viewer, input, at, params),
        ViewerIn::Run(command) => run(viewer, command, at, params),
        ViewerIn::Key(key) => keyed(viewer, &key, at, params),
        // A second close request while the question is up changes nothing.
        ViewerIn::CloseRequested if matches!(viewer.sheet, Sheet::Unsaved(_)) => (viewer, vec![]),
        ViewerIn::CloseRequested => leaving(viewer, Departure::Close, at, params),
        ViewerIn::Saved(end) => saved(viewer, end, at, params),
        ViewerIn::Elapsed => elapsed(viewer, at, params),
    }
}

/// The pointer tool or Space moved. A picture and a PDF have a tool to move; Space coming up is
/// always heard, so a hold is never left behind. The crop tool is a picture's, the markup tool a PDF
/// that can be written on.
pub(super) fn hand(viewer: Viewer, input: HandIn) -> Viewer {
    let on_picture = matches!(viewer.stage, Stage::Raster(_));
    let on_pages = matches!(viewer.stage, Stage::Pdf(_));
    let stepped = |viewer: Viewer| Viewer {
        hand: viewer.hand.step(input),
        ..viewer
    };
    match input {
        HandIn::SpaceUp => stepped(viewer),
        // Only a picture that can be saved with changes has a crop tool.
        HandIn::Use(Tool::Crop) if !(on_picture && viewer.picture.can_edit()) => viewer,
        // Only a PDF that can be marked has a markup tool.
        HandIn::Use(Tool::Markup) if !(on_pages && viewer.markup.can_mark()) => viewer,
        HandIn::Toggle | HandIn::Use(_) | HandIn::SpaceDown if on_picture || on_pages => {
            stepped(viewer)
        }
        HandIn::Toggle | HandIn::Use(_) | HandIn::SpaceDown => viewer,
    }
}

/// An input of the sheet; confirming Move to Trash marks the file as on its way out.
pub(super) fn sheet_in(
    viewer: Viewer,
    input: crate::sheet::SheetIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    // Backing out of the question about a save that loses something leaves the person where they
    // were: nothing is waiting on that save any more.
    let after_save =
        if matches!(viewer.sheet, Sheet::ConfirmEdit { .. }) && input == SheetIn::Cancel {
            None
        } else {
            viewer.after_save.clone()
        };
    let viewer = Viewer {
        after_save,
        ..viewer
    };
    let (viewer, outs) = sheet(viewer, input, at, params);
    let trashed = outs
        .iter()
        .any(|out| matches!(out, ViewerOut::Sheet(SheetOut::Trash)));
    let trashing = if trashed {
        Trashing::Underway
    } else {
        viewer.trashing
    };
    answered(Viewer { trashing, ..viewer }, outs, at, params)
}

/// The context menu's own transitions, then what the row picked did. It opens over a file that is
/// showing and nothing else: a sheet or the palette has the person's attention already.
pub(super) fn context(viewer: Viewer, input: ContextIn, at: Stamp, params: &ViewerParams) -> Step {
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
pub(super) fn navigate(
    viewer: Viewer,
    input: NavigateIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    if walks(&input) {
        leaving(viewer, Departure::Walk(input), at, params)
    } else {
        navigate_now(viewer, input, at, params)
    }
}

/// A move along the sequence that goes ahead whatever is unsaved.
pub(super) fn navigate_now(
    viewer: Viewer,
    input: NavigateIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
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
