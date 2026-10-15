//! Trimming the open recording across regions: Trim… and what it offers, the player kept inside
//! the stretch that plays, the handles and the keys that apply or cancel them, and saving,
//! leaving and taking back a trim.

use super::support::*;
use crate::command::Command;
use crate::edits::{EditRequest, Rewind};
use crate::keys::{Act, Press};
use crate::load::{Load, Ticket};
use crate::picture::{PictureEditIn, PictureEditing};
use crate::sheet::{Departure, MediaOffer, Sheet, SheetIn, SheetParams, Wanted};
use crate::stage::{
    MediaIn, MediaOut, MediaStage, Pace, PlayerCommand, Stage, StageIn, StageOut, TrimEdge,
};
use crate::trim::{MediaTrim, TrimIn};
use crate::viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use anyview_core::{
    Edit, Fact, FactLabel, FactValue, FileAction, MediaExportKind, MediaTime, TimeRange,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};

const fn secs(seconds: u64) -> MediaTime {
    MediaTime::from_secs(seconds)
}

fn held(seconds: u64) -> MediaStage {
    MediaStage::Paused {
        at: secs(seconds),
        length: length(),
    }
}

fn going(seconds: u64) -> MediaStage {
    MediaStage::Playing {
        at: secs(seconds),
        length: length(),
    }
}

fn showing(stage: MediaStage) -> Viewer {
    Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: Stage::Media(stage),
        ..Viewer::default()
    }
}

/// A recording showing with ten to fifty seconds of its minute kept and not saved.
fn trimmed(stage: MediaStage) -> Viewer {
    let trim = MediaTrim::default()
        .step(TrimIn::Begin(length()))
        .step(TrimIn::Move {
            edge: TrimEdge::Start,
            to: secs(10),
        })
        .step(TrimIn::Move {
            edge: TrimEdge::End,
            to: secs(50),
        })
        .step(TrimIn::Apply);
    Viewer {
        trim,
        ..showing(stage)
    }
}

fn needs() -> Fact {
    Fact::new(
        FactLabel::Needs,
        FactValue::text("anyview-ffmpeg (to convert it)"),
    )
}

fn offering(kinds: Vec<MediaExportKind>, needs: Option<Fact>) -> ViewerParams {
    ViewerParams {
        sheet: SheetParams {
            media: MediaOffer::new(kinds, needs),
            ..SheetParams::default()
        },
        ..params()
    }
}

fn step(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(input, Stamp(0), &(), &params())
}

fn press(viewer: Viewer, keys: &[ShortcutKey]) -> (Viewer, Vec<ViewerOut>) {
    step(viewer, ViewerIn::Key(Press::Key(Shortcut(keys.to_vec()))))
}

fn player(command: PlayerCommand) -> ViewerOut {
    ViewerOut::Stage(StageOut::Media(MediaOut::Command(command)))
}

fn media(input: MediaIn) -> ViewerIn {
    ViewerIn::Stage(StageIn::Media(input))
}

const TRIM: ViewerIn = ViewerIn::Run(Command::File(FileAction::Trim));
const SAVE: ViewerIn = ViewerIn::Key(Press::Act(Act::Save));
const CLOSE: ViewerIn = ViewerIn::Key(Press::Act(Act::Close));
const UNDO: ViewerIn = ViewerIn::Key(Press::Act(Act::Undo));
const REDO: ViewerIn = ViewerIn::Key(Press::Act(Act::Redo));

fn the_save() -> ViewerOut {
    ViewerOut::Edit(EditRequest::of_recording(Edit::Trim(
        TimeRange::new(secs(10), Some(secs(50))).unwrap(),
    )))
}

fn saves(outs: &[ViewerOut]) -> bool {
    outs.iter().any(|out| matches!(out, ViewerOut::Edit(_)))
}

#[test]
fn trim_brings_the_bar_up_over_a_held_player_or_names_the_package_that_cuts() {
    let cuts = offering(vec![MediaExportKind::Trim], None);
    let (bar, outs) = showing(going(5)).step(TRIM, Stamp(0), &(), &cuts);
    assert!(bar.trim.is_choosing());
    assert!(
        outs.contains(&player(PlayerCommand::SetPlayback(Pace::Paused))),
        "the player is held while the stretch is chosen"
    );

    let lacks = offering(vec![], Some(needs()));
    let (asked, _) = showing(held(5)).step(TRIM, Stamp(0), &(), &lacks);
    assert_eq!(
        asked.sheet,
        Sheet::Unavailable {
            needs: needs(),
            helper: None,
            wanted: Wanted::Trim
        }
    );
    assert!(!asked.trim.is_choosing());

    let nothing = offering(vec![], None);
    let (same, outs) = showing(held(5)).step(TRIM, Stamp(0), &(), &nothing);
    assert_eq!(
        (same.sheet, same.trim.is_choosing()),
        (Sheet::Closed, false)
    );
    assert!(outs.is_empty());

    let (opening, _) = showing(MediaStage::Opening).step(TRIM, Stamp(0), &(), &cuts);
    assert!(
        !opening.trim.is_choosing(),
        "a recording not open has no length"
    );
}

#[test]
fn the_player_stays_inside_the_stretch_that_is_kept() {
    // The stretch is ten to fifty seconds of a minute; seek steps are 5 s.
    // name, the stage, what happens to it, the stage after
    let cases: Vec<(&str, MediaStage, MediaIn, MediaStage)> = vec![
        (
            "a seek before the start lands on it",
            held(20),
            MediaIn::SeekTo(secs(5)),
            held(10),
        ),
        (
            "a seek past the end lands on it",
            held(20),
            MediaIn::SeekTo(secs(55)),
            held(50),
        ),
        (
            "skipping back stops at the start",
            held(12),
            MediaIn::SeekBack,
            held(10),
        ),
        (
            "skipping forward stops at the end",
            held(48),
            MediaIn::SeekForward,
            held(50),
        ),
        (
            "skipping inside goes as far as it says",
            held(20),
            MediaIn::SeekForward,
            held(25),
        ),
        (
            "play from the end starts again at the start",
            held(50),
            MediaIn::Toggle,
            going(10),
        ),
        (
            "play after the recording ended starts at the start too",
            MediaStage::Ended {
                at: secs(60),
                length: length(),
            },
            MediaIn::Toggle,
            going(10),
        ),
        (
            "play from inside carries on from there",
            held(20),
            MediaIn::Toggle,
            going(20),
        ),
        (
            "playing past the end holds the player at the end",
            going(49),
            MediaIn::Position(secs(51)),
            held(50),
        ),
        (
            "playing inside goes on",
            going(20),
            MediaIn::Position(secs(30)),
            going(30),
        ),
    ];
    for (name, before, input, after) in cases {
        let (viewer, _) = step(trimmed(before), media(input));
        assert_eq!(viewer.stage, Stage::Media(after), "{name}");
    }
}

#[test]
fn a_recording_with_no_trim_plays_where_it_is_asked_to() {
    let (viewer, _) = step(showing(held(20)), media(MediaIn::SeekTo(secs(55))));
    assert_eq!(viewer.stage, Stage::Media(held(55)));
}

#[test]
fn a_handle_shows_its_frame_and_return_and_escape_apply_and_cancel() {
    let cuts = offering(vec![MediaExportKind::Trim], None);
    let (bar, _) = showing(held(5)).step(TRIM, Stamp(0), &(), &cuts);
    let (moved, outs) = step(
        bar,
        ViewerIn::Trim(TrimIn::Move {
            edge: TrimEdge::Start,
            to: secs(20),
        }),
    );
    assert_eq!(
        moved.stage,
        Stage::Media(held(20)),
        "the frame at the handle"
    );
    assert!(outs.contains(&player(PlayerCommand::Seek(secs(20)))));

    let (applied, _) = press(moved.clone(), &[ShortcutKey::Enter]);
    assert!(!applied.trim.is_choosing() && applied.trim.is_edited());
    assert_eq!(applied.trim.applied().map(|w| w.start), Some(secs(20)));
    assert_eq!(applied.stage, Stage::Media(held(20)));

    let (cancelled, _) = press(moved, &[ShortcutKey::Escape]);
    assert!(!cancelled.trim.is_choosing() && !cancelled.trim.is_edited());
}

#[test]
fn space_plays_only_the_selected_stretch_while_the_bar_is_up() {
    let cuts = offering(vec![MediaExportKind::Trim], None);
    let (bar, _) = showing(held(5)).step(TRIM, Stamp(0), &(), &cuts);
    let (bar, _) = step(
        bar,
        ViewerIn::Trim(TrimIn::Move {
            edge: TrimEdge::End,
            to: secs(30),
        }),
    );
    // The handle's frame is at thirty seconds, the end: playing starts over from the start.
    let (playing, _) = press(bar, &[ShortcutKey::Space]);
    assert_eq!(playing.stage, Stage::Media(going(0)));
    let (held_at_end, _) = step(playing, media(MediaIn::Position(secs(31))));
    assert_eq!(held_at_end.stage, Stage::Media(held(30)));
}

#[test]
fn a_trim_is_saved_in_place_and_not_without_one() {
    let (_, outs) = step(trimmed(held(10)), SAVE);
    assert!(outs.contains(&the_save()));
    let (_, outs) = step(showing(held(10)), SAVE);
    assert!(!saves(&outs), "nothing to save");
}

#[test]
fn leaving_a_trimmed_recording_asks_and_each_answer_goes_where_it_should() {
    let (asked, outs) = step(trimmed(held(10)), CLOSE);
    assert_eq!(asked.sheet, Sheet::Unsaved(Departure::Close));
    assert!(!outs.contains(&ViewerOut::CloseWindow));

    let (declined, outs) = step(asked.clone(), ViewerIn::Sheet(SheetIn::Discard));
    assert!(outs.contains(&ViewerOut::CloseWindow), "Don't Save closes");
    assert!(!declined.trim.is_edited(), "and the trim is gone");

    let (cancelled, _) = step(asked.clone(), ViewerIn::Sheet(SheetIn::Cancel));
    assert!(cancelled.trim.is_edited(), "Cancel keeps it");

    let (saving, outs) = step(asked, ViewerIn::Sheet(SheetIn::Confirm));
    assert!(outs.contains(&the_save()));
    assert!(
        !outs.contains(&ViewerOut::CloseWindow),
        "it waits for the file"
    );
    assert_eq!(saving.after_save, Some(Departure::Close));
    // The saved file is read again: the window goes on, with nothing left to save.
    let (landed, outs) = step(
        saving,
        ViewerIn::Picture(PictureEditIn::Landed {
            size: None,
            editing: PictureEditing::Locked,
        }),
    );
    assert!(outs.contains(&ViewerOut::CloseWindow));
    assert!(!landed.trim.is_edited());
}

#[test]
fn undo_takes_the_trim_back_before_it_reaches_the_file() {
    let (undone, outs) = step(trimmed(held(10)), UNDO);
    assert!(!undone.trim.is_edited());
    assert!(!outs.contains(&ViewerOut::Rewind(Rewind::Undo)));
    let (redone, _) = step(undone.clone(), REDO);
    assert!(redone.trim.is_edited());
    // With nothing of the trim to take back, it is the file's last save that is.
    let (_, outs) = step(undone, UNDO);
    assert_eq!(outs, vec![ViewerOut::Rewind(Rewind::Undo)]);
}
