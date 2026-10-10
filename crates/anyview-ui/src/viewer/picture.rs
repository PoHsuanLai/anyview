//! The viewer's side of editing a picture: the edits stepped, and what they change in other
//! regions (the picture fits the window again, the crop tool puts itself away), saving them, and
//! the question asked when the person is about to leave a picture with changes that are not saved.

use super::command::export;
use super::model::{Viewer, ViewerOut, ViewerParams};
use super::region::{Step, sheet, stage};
use super::step::{begin, dropped, navigate_now};
use crate::context::ContextMenu;
use crate::edits::{EditOffer, EditRequest};
use crate::hand::{Hand, Tool};
use crate::palette::Palette;
use crate::picture::PictureEditIn;
use crate::sheet::{
    PictureDeparture, PictureSheet, PictureSheetOut, ResizeDraft, Sheet, SheetIn, SheetOut,
};
use crate::stage::{RasterIn, Stage, StageIn};
use anyview_core::{DocPoint, Edit, Zoom};
use ds_core::time::stamp::Stamp;
use ds_core::vocab::ShortcutKey;

/// An input of the picture edits. When the file has been read again after a save, where the
/// person was going when they chose to save first is where they go now.
pub(super) fn picture_in(
    viewer: Viewer,
    input: PictureEditIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let mut viewer = viewer;
    let going = if matches!(input, PictureEditIn::Landed { .. }) {
        viewer.after_save.take()
    } else {
        // Any other edit is the person carrying on with the picture, not waiting on a save.
        viewer.after_save = None;
        None
    };
    let (viewer, mut outs) = picture(viewer, input, at, params);
    match going {
        Some(departure) => {
            let (viewer, more) = leave(viewer, departure, at, params);
            outs.extend(more);
            (viewer, outs)
        }
        None => (viewer, outs),
    }
}

/// The edits stepped on `input`. A change to what is shown (a turn, a mirror, a cut, taking one
/// back) fits the picture to the window again, since the place the person was zoomed to was in the
/// picture as it was; a cut or a cancel puts the crop tool away.
pub(super) fn picture(
    viewer: Viewer,
    input: PictureEditIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let before = viewer.picture.adjust();
    let was_cropping = viewer.picture.is_cropping();
    let picture = viewer.picture.clone().step(input);
    let moved = picture.adjust() != before;
    let hand = if was_cropping && !picture.is_cropping() && viewer.hand.tool == Tool::Crop {
        Hand {
            tool: Tool::Pan,
            ..viewer.hand
        }
    } else {
        viewer.hand
    };
    let viewer = Viewer {
        picture,
        hand,
        ..viewer
    };
    if moved {
        stage(
            viewer,
            StageIn::Raster(RasterIn::SetZoom {
                zoom: Zoom::Fit,
                at: DocPoint::default(),
            }),
            at,
            params,
        )
    } else {
        (viewer, Vec::new())
    }
}

/// The crop tool and the crop rectangle agree: the tool being Crop on a picture is the rectangle
/// being up, whichever of them moved.
pub(super) fn crop_synced(viewer: Viewer) -> Viewer {
    let wants = viewer.hand.tool == Tool::Crop && matches!(viewer.stage, Stage::Raster(_));
    match (wants, viewer.picture.is_cropping()) {
        (true, false) => Viewer {
            picture: viewer.picture.clone().step(PictureEditIn::CropOn),
            ..viewer
        },
        (false, true) => Viewer {
            picture: viewer.picture.clone().step(PictureEditIn::CropOff),
            ..viewer
        },
        (true, true) | (false, false) => viewer,
    }
}

/// What Enter and Esc mean while the crop rectangle is up and nothing else has the keys.
pub(super) fn crop_key(viewer: &Viewer, keys: &[ShortcutKey]) -> Option<PictureEditIn> {
    let free = matches!(viewer.sheet, Sheet::Closed)
        && matches!(viewer.palette, Palette::Closed)
        && matches!(viewer.context, ContextMenu::Closed);
    if !free || !viewer.picture.is_cropping() {
        return None;
    }
    match keys {
        [ShortcutKey::Enter] => Some(PictureEditIn::Apply),
        [ShortcutKey::Escape] => Some(PictureEditIn::CropOff),
        _ => None,
    }
}

/// ⌘S: the changes are written into the file. A picture that cannot be written offers the Export
/// dialog instead, as Preview offers a copy; one with nothing to save does nothing. A save that
/// loses something is asked about first.
pub(super) fn save(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    if !matches!(viewer.stage, Stage::Raster(_)) {
        return (viewer, Vec::new());
    }
    if !viewer.picture.is_edited() {
        return if viewer.picture.can_edit() {
            (viewer, Vec::new())
        } else {
            export(viewer, at, params)
        };
    }
    let request = EditRequest::of_picture(Edit::Adjust(viewer.picture.adjust()));
    match params.sheet.edit {
        EditOffer::Plain => (viewer, vec![ViewerOut::Edit(request)]),
        EditOffer::Asks(caution) => sheet(viewer, SheetIn::AskEdit(request, caution), at, params),
        EditOffer::Withheld => export(viewer, at, params),
    }
}

/// Adjust Size…: the sheet that chooses the picture's new size, starting from the size it has.
pub(super) fn adjust_size(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    match viewer.picture.result_size() {
        Some(size) if viewer.picture.can_edit() => sheet(
            viewer,
            SheetIn::AskPicture(PictureSheet::Resize(ResizeDraft::of(size))),
            at,
            params,
        ),
        Some(_) | None => (viewer, Vec::new()),
    }
}

/// Whether going to another file or closing the window would lose changes to the picture.
fn unsaved(viewer: &Viewer) -> bool {
    viewer.picture.is_edited() && matches!(viewer.stage, Stage::Raster(_))
}

/// Go where `departure` says, or first ask what to do with changes that are not saved.
pub(super) fn leaving(
    viewer: Viewer,
    departure: PictureDeparture,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    if unsaved(&viewer) {
        sheet(
            viewer,
            SheetIn::AskPicture(PictureSheet::Unsaved(departure)),
            at,
            params,
        )
    } else {
        leave(viewer, departure, at, params)
    }
}

/// Go where `departure` says.
fn leave(viewer: Viewer, departure: PictureDeparture, at: Stamp, params: &ViewerParams) -> Step {
    match departure {
        PictureDeparture::Close => (viewer, vec![ViewerOut::CloseWindow]),
        PictureDeparture::Open(path) => begin(viewer, &path, at, params),
        PictureDeparture::Chosen(paths) | PictureDeparture::Dropped(paths) => {
            dropped(viewer, paths, at, params)
        }
        PictureDeparture::Walk(input) => navigate_now(viewer, input, at, params),
    }
}

/// Whether `input` walks to another file, so that it would leave the picture.
pub(super) fn walks(input: &crate::navigate::NavigateIn) -> bool {
    use crate::navigate::NavigateIn;
    match input {
        NavigateIn::Next | NavigateIn::Previous | NavigateIn::First | NavigateIn::Last => true,
        NavigateIn::Start(_) | NavigateIn::Leave | NavigateIn::Gone | NavigateIn::Elapsed => false,
    }
}

/// A sheet of editing a picture was answered: a new size is applied, Save writes the changes (and
/// the person goes on once the saved file has been read again), Don't Save lets them go and goes
/// on at once.
pub(super) fn answered(
    viewer: Viewer,
    outs: Vec<ViewerOut>,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let answers: Vec<PictureSheetOut> = outs
        .iter()
        .filter_map(|out| {
            if let ViewerOut::Sheet(SheetOut::Picture(answer)) = out {
                Some(answer.clone())
            } else {
                None
            }
        })
        .collect();
    let mut state = (viewer, outs);
    for answer in answers {
        let (viewer, mut outs) = state;
        let (viewer, more) = match answer {
            PictureSheetOut::Resize(size) => {
                picture(viewer, PictureEditIn::Resize(size), at, params)
            }
            PictureSheetOut::Save(then) => {
                let (viewer, more) = save(viewer, at, params);
                (
                    Viewer {
                        after_save: Some(then),
                        ..viewer
                    },
                    more,
                )
            }
            PictureSheetOut::Discard(then) => {
                let (viewer, mut first) = picture(viewer, PictureEditIn::Discard, at, params);
                let (viewer, second) = leave(viewer, then, at, params);
                first.extend(second);
                (viewer, first)
            }
        };
        outs.extend(more);
        state = (viewer, outs);
    }
    state
}
