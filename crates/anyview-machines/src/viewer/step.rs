//! The root's transitions: route an input to its region, lift the outputs, and handle what
//! crosses regions.

use super::command::run;
use super::departure::{leaving, save_open, saved, walks};
use super::model::{Choosing, PanelSay, Trashing, Viewer, ViewerIn, ViewerOut, ViewerParams};
use super::picture::{answered, crop_key, crop_synced, picture, picture_in};
use super::pins::{synced, wanted};
use super::region::{Step, chrome, panel, presentation, sheet, stage, stepped};
use crate::command::{Command, StageCommand};
use crate::context::{ContextIn, ContextOut, ContextPick};
use crate::edits::Rewind;
use crate::hand::{HandIn, Tool};
use crate::keys::{Act, Chords, Press, Regions, Route, route};
use crate::load::Ticket;
use crate::load::{Load, LoadFailure, LoadIn, LoadOut};
use crate::navigate::{Navigate, NavigateIn, NavigateOut};
use crate::palette::{Palette, PaletteIn, PaletteIndex, PaletteOut, PaletteScope, step_with_scope};
use crate::panel::{Panel, PanelIn, PanelOut, PanelTab};
use crate::picture::{PictureEditIn, PictureEdits};
use crate::presentation::Presentation;
use crate::sheet::{Departure, Sheet, SheetIn, SheetOut};
use crate::stage::{Stage, StageFamily, StageIn, TextIn};
use crate::typed::TypedText;
use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};
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
        let (viewer, mut outs) = apply(self, input, at, params);
        let (viewer, pinned) = synced(viewer, &before, at, params);
        outs.extend(pinned);
        (crop_synced(viewer), outs)
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
        ViewerIn::Run(command) => run(viewer, command, at, params),
        ViewerIn::Key(key) => keyed(viewer, &key, at, params),
        // A second close request while the question is up changes nothing.
        ViewerIn::CloseRequested if matches!(viewer.sheet, Sheet::Unsaved(_)) => (viewer, vec![]),
        ViewerIn::CloseRequested => leaving(viewer, Departure::Close, at, params),
        ViewerIn::Saved(end) => saved(viewer, end, at, params),
        ViewerIn::Elapsed => elapsed(viewer, at, params),
    }
}

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
    // about the picture left; a held Space and the picture's place are about the file left too.
    let mut hand = viewer.hand.step(HandIn::SpaceUp);
    if hand.tool == Tool::Crop {
        hand.tool = Tool::Pan;
    }
    let viewer = Viewer {
        stage: Stage::NoStage,
        trashing: Trashing::Not,
        hand,
        picture: PictureEdits::default(),
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

/// The pan tool or Space moved. Only a picture has one to move; Space coming up is always heard, so
/// a hold is never left behind.
pub(super) fn hand(viewer: Viewer, input: HandIn) -> Viewer {
    let pans_here = matches!(viewer.stage, Stage::Raster(_));
    match (input, pans_here) {
        // Only a picture that can be saved with changes has a crop tool.
        (HandIn::Use(Tool::Crop), _) if !(pans_here && viewer.picture.can_edit()) => viewer,
        (HandIn::SpaceUp, _) | (HandIn::Toggle | HandIn::Use(_) | HandIn::SpaceDown, true) => {
            Viewer {
                hand: viewer.hand.step(input),
                ..viewer
            }
        }
        (HandIn::Toggle | HandIn::Use(_) | HandIn::SpaceDown, false) => viewer,
    }
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

/// An input of the sheet; confirming Move to Trash marks the file as on its way out.
fn sheet_in(
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

/// The stage for a file of `family`: the one showing when it is already of that family (a reload
/// keeps its place), otherwise a new one.
fn kept_or_new(showing: &Stage, family: StageFamily, params: &ViewerParams) -> Stage {
    if showing.family() == family {
        showing.clone()
    } else {
        Stage::for_family(family, params.stage.text.views).wrapped(params.stage.text.wrap)
    }
}

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
fn searched(viewer: Viewer, query: &TypedText, at: Stamp, params: &ViewerParams) -> Step {
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

fn keyed(viewer: Viewer, key: &Press, at: Stamp, params: &ViewerParams) -> Step {
    if let Some(input) = crop_key(&viewer, key.keys().as_slice()) {
        return picture(viewer, input, at, params);
    }
    let routed = route(
        key,
        Regions {
            sheet: &viewer.sheet,
            palette: &viewer.palette,
            context: &viewer.context,
            panel: &viewer.panel,
            stage: &viewer.stage,
            stage_params: &params.stage,
            platform: params.platform,
            chords: match viewer.presentation {
                Presentation::Pane => Chords::None,
                Presentation::Window
                | Presentation::Peek
                | Presentation::Mini
                | Presentation::Background => Chords::Viewer,
            },
        },
    );
    match routed {
        Route::Sheet(input) => sheet_in(viewer, input, at, params),
        Route::Palette(input) => palette(viewer, input, at, params),
        Route::OpenPalette => palette(viewer, PaletteIn::Open, at, params),
        Route::OpenFind => open_find(viewer, at, params),
        Route::Context(input) => context(viewer, input, at, params),
        Route::OpenContextMenu => context(viewer, ContextIn::OpenAtCentre, at, params),
        Route::Panel(input) => panel(viewer, input, at, params),
        Route::CloseWindow => leaving(viewer, Departure::Close, at, params),
        Route::Save => save_open(viewer, at, params),
        Route::OpenFile => choose(viewer),
        Route::Rewind(rewind) => rewound(viewer, rewind, at, params),
        Route::Dismiss => dismissed(viewer, at, params),
        // Done and Save of a text being edited go through the command, which asks first where a
        // question is due.
        Route::Stage(StageIn::Text(TextIn::Done)) => {
            run(viewer, Command::Stage(StageCommand::Done), at, params)
        }
        Route::Stage(StageIn::Text(TextIn::Save)) => {
            run(viewer, Command::Stage(StageCommand::Save), at, params)
        }
        Route::Stage(input) => stage(viewer, input, at, params),
        Route::Hand(input) => (hand(viewer, input), vec![]),
        Route::Navigate(input) => navigate(viewer, input, at, params),
        Route::Chrome(input) => chrome(viewer, input, at, params),
        Route::Ignored => file_key(viewer, key, at, params),
        Route::Swallowed => (viewer, vec![]),
    }
}

/// A press no region claimed, which may be the action of a file action the open file offers (Print,
/// Duplicate, Rotate…): it runs as the palette's row would.
fn file_key(viewer: Viewer, key: &Press, at: Stamp, params: &ViewerParams) -> Step {
    match key.act() {
        Some(Act::File(action)) if params.files.contains(&action) => {
            run(viewer, Command::File(action), at, params)
        }
        Some(_) | None => (viewer, vec![]),
    }
}

/// ⌘Z and ⇧⌘Z: an edit of the picture that is not saved is taken back or done again first; with
/// none, it is the file's last save that is.
fn rewound(viewer: Viewer, rewind: Rewind, at: Stamp, params: &ViewerParams) -> Step {
    let input = match rewind {
        Rewind::Undo if viewer.picture.can_undo() => Some(PictureEditIn::Undo),
        Rewind::Redo if viewer.picture.can_redo() => Some(PictureEditIn::Redo),
        Rewind::Undo | Rewind::Redo => None,
    };
    match input {
        Some(input) => picture(viewer, input, at, params),
        None => (viewer, vec![ViewerOut::Rewind(rewind)]),
    }
}

/// Esc with nothing open: a quick look closes; a window, a mini window and a background session
/// stay; a pane gives the keyboard back to its host and stays.
fn dismissed(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    match viewer.presentation {
        Presentation::Peek => leaving(viewer, Departure::Close, at, params),
        Presentation::Pane => (viewer, vec![ViewerOut::Unfocus]),
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
