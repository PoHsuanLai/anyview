//! The open picture's edits across regions: what leaving, closing and saving do while changes are
//! waiting, how the crop tool follows the tool and the keys, and how ⌘Z reaches the edits before it
//! reaches the file.

use super::support::*;
use crate::command::{Command, PictureCommand};
use crate::edits::{EditCaution, EditOffer, EditRequest, Rewind};
use crate::hand::Tool;
use crate::keys::{Act, Press};
use crate::load::{Load, Ticket};
use crate::navigate::NavigateIn;
use crate::picture::{PictureEditIn, PictureEditing};
use crate::sheet::{
    Departure, ExportDraft, PictureSheet, PictureSheetIn, ResizeChange, ResizeDraft, Sheet,
    SheetIn, SheetOut, SheetParams,
};
use crate::stage::Stage;
use crate::viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use anyview_core::{
    Adjust, Edit, FilePath, NonEmpty, PixelLen, PixelSize, QuarterTurn, RasterExport, RasterTarget,
    Sequence, SequenceOrigin,
};
use ds_core::machine::Machine;
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

/// A picture showing, scaled smaller and not saved.
fn resized() -> Viewer {
    Viewer {
        picture: editable().step(PictureEditIn::Resize(size(200, 150))),
        ..turned()
    }
}

const LOST: EditCaution = EditCaution::Loses("Only the first frame will be kept.");

/// A file whose save loses something: leaving it with changes asks.
fn losing() -> ViewerParams {
    ViewerParams {
        sheet: SheetParams {
            edit: EditOffer::Asks(LOST),
            ..SheetParams::default()
        },
        ..params()
    }
}

fn step(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(input, Stamp(0), &(), &params())
}

fn step_losing(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(input, Stamp(0), &(), &losing())
}

fn press(viewer: Viewer, keys: &[ShortcutKey]) -> (Viewer, Vec<ViewerOut>) {
    step(viewer, ViewerIn::Key(Press::Key(Shortcut(keys.to_vec()))))
}

fn press_losing(viewer: Viewer, keys: &[ShortcutKey]) -> (Viewer, Vec<ViewerOut>) {
    step_losing(viewer, ViewerIn::Key(Press::Key(Shortcut(keys.to_vec()))))
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

fn the_resize() -> EditRequest {
    EditRequest::of_picture(Edit::Adjust(Adjust {
        size: Some(size(200, 150)),
        ..Adjust::NONE
    }))
}

const CLOSE: ViewerIn = ViewerIn::Key(Press::Act(Act::Close));
const SAVE: ViewerIn = ViewerIn::Key(Press::Act(Act::Save));

#[test]
fn closing_a_picture_whose_save_loses_something_asks_and_each_answer_goes_where_it_should() {
    let (asked, outs) = step_losing(resized(), CLOSE);
    assert_eq!(asked.sheet, Sheet::Unsaved(Departure::Close));
    assert!(!closes(&outs), "the window waits for the answer");

    let (cancelled, outs) = step_losing(asked.clone(), ViewerIn::Sheet(SheetIn::Cancel));
    assert_eq!(cancelled.sheet, Sheet::Closed);
    assert!(cancelled.picture.is_edited(), "Cancel keeps the changes");
    assert!(!closes(&outs));

    let (declined, outs) = step_losing(asked.clone(), ViewerIn::Sheet(SheetIn::Discard));
    assert!(closes(&outs), "Don't Save closes at once");
    assert!(!declined.picture.is_edited(), "and the changes are gone");

    let (saving, outs) = step_losing(asked, ViewerIn::Sheet(SheetIn::Confirm));
    assert_eq!(
        saving.sheet,
        Sheet::ConfirmEdit {
            request: the_resize(),
            caution: LOST,
        },
        "Save says what the new size loses"
    );
    assert!(
        !closes(&outs),
        "and the window closes once the file is read again"
    );
    assert_eq!(saving.after_save, Some(Departure::Close));
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
fn a_turned_or_resized_picture_is_kept_without_asking_when_the_person_goes_on() {
    for (change, changed, written) in [
        ("turned", turned(), the_save()),
        ("resized", resized(), the_resize()),
    ] {
        let files = NonEmpty::from_vec(vec![path("/a.png"), path("/b.png")]).unwrap();
        let sequence =
            Sequence::starting_at(files, &path("/a.png"), SequenceOrigin::Selection).unwrap();
        let (walking, _) = step(changed, ViewerIn::Navigate(NavigateIn::Start(sequence)));
        let landed = ViewerIn::Picture(PictureEditIn::Landed {
            size: Some(size(300, 400)),
            editing: PictureEditing::Allowed,
        });
        // name, what the person does, where they go once the file has been written
        let cases = [
            ("Close", CLOSE, Departure::Close),
            (
                "an arrow key",
                ViewerIn::Key(Press::Key(Shortcut(vec![ShortcutKey::Right]))),
                Departure::Walk(NavigateIn::Next),
            ),
            (
                "opening another file",
                ViewerIn::Open(path("/b.png")),
                Departure::Open(path("/b.png")),
            ),
        ];
        for (name, input, then) in cases {
            let (saving, outs) = step(walking.clone(), input);
            assert_eq!(saving.sheet, Sheet::Closed, "{change}, {name}: no question");
            assert!(
                outs.contains(&ViewerOut::Edit(written)),
                "{change}, {name}: written"
            );
            assert_eq!(
                saving.after_save,
                Some(then),
                "{change}, {name}: goes on afterwards"
            );
            // A key held down walks again while the write is on its way: nothing is written twice.
            let (waiting, outs) = press(saving.clone(), &[ShortcutKey::Right]);
            assert!(
                !outs.iter().any(|out| matches!(out, ViewerOut::Edit(_))),
                "{change}, {name}: not written twice"
            );
            assert_eq!(
                waiting.after_save, saving.after_save,
                "{change}, {name}: still going on"
            );
            let (_, outs) = step(saving, landed.clone());
            assert!(
                closes(&outs) || probes(&outs, "/b.png"),
                "{change}, {name}: goes on once the file is read again"
            );
        }
    }
}

#[test]
fn a_turn_that_loses_something_is_still_asked_about_on_leaving() {
    let (viewer, outs) = step_losing(turned(), CLOSE);
    assert_eq!(viewer.sheet, Sheet::Unsaved(Departure::Close));
    assert!(!outs.iter().any(|out| matches!(out, ViewerOut::Edit(_))));
}

#[test]
fn a_picture_with_nothing_unsaved_closes_without_asking() {
    let viewer = Viewer {
        picture: editable(),
        ..turned()
    };
    let (viewer, outs) = step(viewer, CLOSE);
    assert!(closes(&outs));
    assert_eq!(viewer.sheet, Sheet::Closed);
}

#[test]
fn opening_another_file_when_the_save_loses_something_asks_first_and_not_saving_goes_on_to_it() {
    let (asked, outs) = step_losing(resized(), ViewerIn::Open(path("/b.png")));
    assert_eq!(asked.sheet, Sheet::Unsaved(Departure::Open(path("/b.png"))));
    assert!(
        !probes(&outs, "/b.png"),
        "nothing is opened before the answer"
    );
    let (declined, outs) = step_losing(asked, ViewerIn::Sheet(SheetIn::Discard));
    assert!(probes(&outs, "/b.png"));
    assert!(!declined.picture.is_edited());
}

#[test]
fn an_arrow_key_asks_before_leaving_a_picture_whose_save_loses_something_and_does_not_move_the_walk()
 {
    let files = NonEmpty::from_vec(vec![path("/a.png"), path("/b.png")]).unwrap();
    let sequence =
        Sequence::starting_at(files, &path("/a.png"), SequenceOrigin::Selection).unwrap();
    let (walking, _) = step_losing(resized(), ViewerIn::Navigate(NavigateIn::Start(sequence)));
    let (asked, outs) = press_losing(walking.clone(), &[ShortcutKey::Right]);
    assert_eq!(
        asked.sheet,
        Sheet::Unsaved(Departure::Walk(NavigateIn::Next))
    );
    assert_eq!(asked.navigate, walking.navigate, "the walk has not moved");
    assert!(!probes(&outs, "/b.png"));
    let (_, outs) = step_losing(asked, ViewerIn::Sheet(SheetIn::Discard));
    assert!(probes(&outs, "/b.png"));
}

#[test]
fn the_save_chord_writes_the_whole_adjustment() {
    let (_, outs) = step(turned(), SAVE);
    assert_eq!(outs, vec![ViewerOut::Edit(the_save())]);
    let (_, outs) = step(
        Viewer {
            picture: editable(),
            ..turned()
        },
        SAVE,
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
    let (viewer, _) = viewer.step(SAVE, Stamp(0), &(), &params);
    assert!(matches!(viewer.sheet, Sheet::Export { .. }));
}

#[test]
fn a_picture_whose_file_has_no_writer_is_turned_and_saved_as_a_copy_and_leaving_asks_first() {
    let params = ViewerParams {
        sheet: SheetParams {
            edit: EditOffer::Copies(RasterTarget::Png),
            ..SheetParams::default()
        },
        ..params()
    };
    let copy = ViewerOut::Sheet(SheetOut::Export(ExportDraft::Raster(
        RasterExport::Adjusted(RasterTarget::Png, Adjust::NONE.turned(QuarterTurn::Quarter)),
    )));
    let run = |viewer: Viewer, input: ViewerIn| viewer.step(input, Stamp(0), &(), &params);

    // Leaving is asked about, never kept without a question: the file cannot take the turn.
    let (asked, outs) = run(turned(), CLOSE);
    assert_eq!(asked.sheet, Sheet::Unsaved(Departure::Close));
    assert!(!closes(&outs));
    // Save writes the copy and lets the changes go; the file is not read again, so nothing waits.
    let (saved, outs) = run(asked, ViewerIn::Sheet(SheetIn::Confirm));
    assert!(outs.contains(&copy));
    assert!(!saved.picture.is_edited());
    assert_eq!(saved.after_save, None);
    assert!(!closes(&outs));
    // So does the save chord.
    let (saved, outs) = run(turned(), SAVE);
    assert!(outs.contains(&copy));
    assert!(!saved.picture.is_edited());
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
    let undo = ViewerIn::Key(Press::Act(Act::Undo));
    let redo = ViewerIn::Key(Press::Act(Act::Redo));
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
