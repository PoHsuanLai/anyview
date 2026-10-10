//! The open picture's edits across regions: what leaving, closing and saving do while changes are
//! waiting, how the crop tool follows the tool and the keys, and how ⌘Z reaches the edits before it
//! reaches the file.

use super::support::*;
use crate::command::{Command, PictureCommand};
use crate::edits::{EditCaution, EditOffer, EditRequest, Rewind};
use crate::hand::Tool;
use crate::load::{Load, Ticket};
use crate::navigate::NavigateIn;
use crate::picture::{PictureEditIn, PictureEditing};
use crate::sheet::{
    PictureDeparture, PictureSheet, PictureSheetIn, ResizeChange, ResizeDraft, Sheet, SheetIn,
    SheetParams,
};
use crate::stage::Stage;
use crate::viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use anyview_core::{
    Adjust, Edit, FilePath, NonEmpty, PixelLen, PixelSize, QuarterTurn, Sequence, SequenceOrigin,
};
use ds_core::machine::Machine;
use ds_core::standard_action::StandardAction;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    }
}

/// A picture showing, turned a quarter and not saved.
fn turned() -> Viewer {
    Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: image(),
        picture: editable().step(PictureEditIn::Turn(QuarterTurn::Quarter)),
        ..Viewer::default()
    }
}

fn step(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(input, Stamp(0), &(), &params())
}

fn press(viewer: Viewer, keys: &[ShortcutKey]) -> (Viewer, Vec<ViewerOut>) {
    step(viewer, ViewerIn::Key(Shortcut(keys.to_vec())))
}

/// Whether the viewer asked to open `file`.
fn probes(outs: &[ViewerOut], file: &str) -> bool {
    outs.iter()
        .any(|out| matches!(out, ViewerOut::Probe { path, .. } if *path == self::path(file)))
}

fn closes(outs: &[ViewerOut]) -> bool {
    outs.contains(&ViewerOut::CloseWindow)
}

fn the_save() -> EditRequest {
    EditRequest::of_picture(Edit::Adjust(Adjust::NONE.turned(QuarterTurn::Quarter)))
}

const COMMAND_W: [ShortcutKey; 2] = [ShortcutKey::Super, ShortcutKey::Char('w')];
const COMMAND_S: [ShortcutKey; 2] = [ShortcutKey::Super, ShortcutKey::Char('s')];

#[test]
fn closing_a_picture_with_changes_asks_and_each_answer_goes_where_it_should() {
    let (asked, outs) = press(turned(), &COMMAND_W);
    assert_eq!(
        asked.sheet,
        Sheet::Picture(PictureSheet::Unsaved(PictureDeparture::Close))
    );
    assert!(!closes(&outs), "the window waits for the answer");

    let (cancelled, outs) = step(asked.clone(), ViewerIn::Sheet(SheetIn::Cancel));
    assert_eq!(cancelled.sheet, Sheet::Closed);
    assert!(cancelled.picture.is_edited(), "Cancel keeps the changes");
    assert!(!closes(&outs));

    let (declined, outs) = step(
        asked.clone(),
        ViewerIn::Sheet(SheetIn::Picture(PictureSheetIn::Decline)),
    );
    assert!(closes(&outs), "Don't Save closes at once");
    assert!(!declined.picture.is_edited(), "and the changes are gone");

    let (saving, outs) = step(asked, ViewerIn::Sheet(SheetIn::Confirm));
    assert!(
        outs.contains(&ViewerOut::Edit(the_save())),
        "Save writes the turn"
    );
    assert!(
        !closes(&outs),
        "and the window closes once the file is read again"
    );
    assert_eq!(saving.after_save, Some(PictureDeparture::Close));
    let (done, outs) = step(
        saving,
        ViewerIn::Picture(PictureEditIn::Landed {
            size: Some(size(400, 300)),
            editing: PictureEditing::Allowed,
        }),
    );
    assert!(closes(&outs));
    assert_eq!(done.after_save, None);
}

#[test]
fn a_picture_with_nothing_unsaved_closes_without_asking() {
    let viewer = Viewer {
        picture: editable(),
        ..turned()
    };
    let (viewer, outs) = press(viewer, &COMMAND_W);
    assert!(closes(&outs));
    assert_eq!(viewer.sheet, Sheet::Closed);
}

#[test]
fn opening_another_file_asks_first_and_not_saving_goes_on_to_it() {
    let (asked, outs) = step(turned(), ViewerIn::Open(path("/b.png")));
    assert_eq!(
        asked.sheet,
        Sheet::Picture(PictureSheet::Unsaved(PictureDeparture::Open(path(
            "/b.png"
        ))))
    );
    assert!(
        !probes(&outs, "/b.png"),
        "nothing is opened before the answer"
    );
    let (declined, outs) = step(
        asked,
        ViewerIn::Sheet(SheetIn::Picture(PictureSheetIn::Decline)),
    );
    assert!(probes(&outs, "/b.png"));
    assert!(!declined.picture.is_edited());
}

#[test]
fn an_arrow_key_asks_before_leaving_the_picture_and_does_not_move_the_walk() {
    let files = NonEmpty::from_vec(vec![path("/a.png"), path("/b.png")]).unwrap();
    let sequence =
        Sequence::starting_at(files, &path("/a.png"), SequenceOrigin::Selection).unwrap();
    let (walking, _) = step(turned(), ViewerIn::Navigate(NavigateIn::Start(sequence)));
    let (asked, outs) = press(walking.clone(), &[ShortcutKey::Right]);
    assert_eq!(
        asked.sheet,
        Sheet::Picture(PictureSheet::Unsaved(PictureDeparture::Walk(
            NavigateIn::Next
        )))
    );
    assert_eq!(asked.navigate, walking.navigate, "the walk has not moved");
    assert!(!probes(&outs, "/b.png"));
    let (_, outs) = step(
        asked,
        ViewerIn::Sheet(SheetIn::Picture(PictureSheetIn::Decline)),
    );
    assert!(probes(&outs, "/b.png"));
}

#[test]
fn the_save_chord_writes_the_whole_adjustment() {
    let (_, outs) = press(turned(), &COMMAND_S);
    assert_eq!(outs, vec![ViewerOut::Edit(the_save())]);
    let (_, outs) = step(
        Viewer {
            picture: editable(),
            ..turned()
        },
        ViewerIn::Key(Shortcut(COMMAND_S.to_vec())),
    );
    assert!(
        outs.is_empty(),
        "with nothing changed there is nothing to write"
    );
}

#[test]
fn a_save_that_loses_something_asks_first() {
    let caution = EditCaution::Loses("Only the first frame will be kept.");
    let params = ViewerParams {
        sheet: SheetParams {
            edit: EditOffer::Asks(caution),
            ..SheetParams::default()
        },
        ..params()
    };
    let (viewer, outs) = turned().step(
        ViewerIn::Run(Command::Picture(PictureCommand::Save)),
        Stamp(0),
        &(),
        &params,
    );
    assert_eq!(
        viewer.sheet,
        Sheet::ConfirmEdit {
            request: the_save(),
            caution
        }
    );
    assert!(!outs.iter().any(|out| matches!(out, ViewerOut::Edit(_))));
}

#[test]
fn the_save_chord_on_a_picture_that_cannot_be_written_offers_the_export_dialog() {
    let params = ViewerParams {
        sheet: SheetParams {
            edit: EditOffer::Withheld,
            ..SheetParams::default()
        },
        ..params()
    };
    let viewer = Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: image(),
        ..Viewer::default()
    };
    let (viewer, _) = viewer.step(
        ViewerIn::Key(Shortcut(COMMAND_S.to_vec())),
        Stamp(0),
        &(),
        &params,
    );
    assert!(matches!(viewer.sheet, Sheet::Export { .. }));
}

#[test]
fn the_crop_tool_and_the_rectangle_go_up_and_away_together() {
    let showing = Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: image(),
        picture: editable(),
        ..Viewer::default()
    };
    // Choosing the tool draws the rectangle; Enter on the whole picture cuts nothing and puts both away.
    let (cropping, _) = step(showing.clone(), ViewerIn::Run(Command::UseTool(Tool::Crop)));
    assert_eq!(cropping.hand.tool, Tool::Crop);
    assert!(cropping.picture.is_cropping());
    let (done, _) = press(cropping.clone(), &[ShortcutKey::Enter]);
    assert!(!done.picture.is_cropping());
    assert_eq!(done.hand.tool, Tool::Pan);
    assert!(!done.picture.is_edited());
    // Esc puts them away too, and the picture is as it was.
    let (cancelled, _) = press(cropping.clone(), &[ShortcutKey::Escape]);
    assert!(!cancelled.picture.is_cropping());
    assert_eq!(cancelled.hand.tool, Tool::Pan);
    // Another tool drops the rectangle.
    let (other, _) = step(cropping, ViewerIn::Run(Command::UseTool(Tool::Select)));
    assert!(!other.picture.is_cropping());
    // The C key chooses the tool.
    let (keyed, _) = press(showing, &[ShortcutKey::Char('c')]);
    assert!(keyed.picture.is_cropping());
}

#[test]
fn a_picture_that_cannot_be_saved_has_no_crop_tool() {
    let viewer = Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: image(),
        ..Viewer::default()
    };
    let (viewer, _) = step(viewer, ViewerIn::Run(Command::UseTool(Tool::Crop)));
    assert_eq!(viewer.hand.tool, Tool::Pan);
    assert!(!viewer.picture.is_cropping());
}

#[test]
fn a_new_file_does_not_carry_the_crop_tool_along() {
    let showing = Viewer {
        stage: image(),
        picture: editable(),
        ..Viewer::default()
    };
    let (cropping, _) = step(showing, ViewerIn::Run(Command::UseTool(Tool::Crop)));
    let (next, _) = step(cropping, ViewerIn::Open(path("/b.png")));
    assert_eq!(next.hand.tool, Tool::Pan);
    assert!(!next.picture.is_cropping());
}

#[test]
fn undo_takes_back_an_edit_that_is_not_saved_before_it_asks_the_host_to_undo_a_save() {
    let undo = ViewerIn::Key(Shortcut::standard(StandardAction::Undo));
    let redo = ViewerIn::Key(Shortcut::standard(StandardAction::Redo));
    let (undone, outs) = step(turned(), undo.clone());
    assert!(!undone.picture.is_edited());
    assert_eq!(outs, vec![refitted()], "no request of the host");
    let (redone, _) = step(undone.clone(), redo.clone());
    assert!(redone.picture.is_edited());
    // With nothing waiting, it is the file's last save that goes.
    let (_, outs) = step(undone, undo);
    assert_eq!(outs, vec![ViewerOut::Rewind(Rewind::Undo)]);
    let (_, outs) = step(redone, redo);
    assert_eq!(outs, vec![ViewerOut::Rewind(Rewind::Redo)]);
}

#[test]
fn adjust_size_opens_on_the_size_the_picture_has_and_ok_applies_what_was_chosen() {
    let showing = Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: image(),
        picture: editable(),
        ..Viewer::default()
    };
    let (asked, _) = step(
        showing,
        ViewerIn::Run(Command::Picture(PictureCommand::AdjustSize)),
    );
    assert_eq!(
        asked.sheet,
        Sheet::Picture(PictureSheet::Resize(ResizeDraft::of(size(400, 300))))
    );
    let (typed, _) = step(
        asked,
        ViewerIn::Sheet(SheetIn::Picture(PictureSheetIn::Resize(
            ResizeChange::Width(200),
        ))),
    );
    let (done, _) = step(typed, ViewerIn::Sheet(SheetIn::Confirm));
    assert_eq!(done.sheet, Sheet::Closed);
    assert_eq!(done.picture.adjust().size, Some(size(200, 150)));
    assert!(done.picture.is_edited());
}

#[test]
fn a_picture_landing_forgets_the_changes_made_to_the_last_one() {
    let (landed, _) = step(
        turned(),
        ViewerIn::Picture(PictureEditIn::Landed {
            size: Some(size(10, 10)),
            editing: PictureEditing::Allowed,
        }),
    );
    assert!(!landed.picture.is_edited());
    assert_eq!(landed.picture.base(), Some(size(10, 10)));
    assert_eq!(
        landed.stage,
        Stage::Raster(crate::stage::RasterStage::default())
    );
}
