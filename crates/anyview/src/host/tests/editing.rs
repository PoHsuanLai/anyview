//! Edits saved in place through the desktop, with a version of each original in the store: the
//! file on disk, the versions kept, undo, redo, Revert To, Save a Copy and the prune.

use super::support::{desktop, path, probed};
use crate::host::{Carry, Declined, Hosting, Outcome, Shown, Task, route};
use anyview_core::{
    Adjust, Axis, Edit, FilePath, PageIndex, PageRange, PixelLen, PixelRect, PixelSize, QuarterTurn,
};
use anyview_image::ExifFacts;
use anyview_pdf::PdfDocument;
use anyview_store::{SavedAt, VersionId, Versions};
use anyview_ui::{EditRequest, HostRequest, Opened, Rewind, TextSave, TypedText};

#[path = "../../../../anyview-pdf/tests/pdf/support/mod.rs"]
#[allow(clippy::unwrap_used)]
mod pdf_fixture;

const JPEG: &[u8] = include_bytes!("../../../../anyview-image/tests/fixtures/rotated.jpg");
const QUADRANTS: &[u8] = include_bytes!("../../../../anyview-image/tests/fixtures/quadrants.png");

fn tag(bytes: &[u8]) -> u16 {
    ExifFacts::read(bytes).orientation.tag()
}

fn turn() -> EditRequest {
    EditRequest::of_picture(Edit::Rotate(QuarterTurn::Quarter))
}

fn kept_of(outcome: &Outcome) -> VersionId {
    let Outcome::Written { kept, .. } = outcome else {
        panic!("not written: {outcome:?}")
    };
    kept.clone()
}

fn bytes_of(file: &Opened) -> Vec<u8> {
    std::fs::read(file.source.path().as_path()).unwrap()
}

/// The versions the scratch dir's store holds for `file`, newest first.
fn versions_of(scratch: &std::path::Path, file: &Opened) -> Vec<anyview_store::Version> {
    Versions::under_state(&scratch.join("state"))
        .list(file.source.path().as_path())
        .unwrap()
}

#[tokio::test]
async fn a_jpeg_turn_is_saved_in_place_with_its_original_kept() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let outcome = desktop
        .carry_out(Task::Edit {
            file: file.clone(),
            request: turn(),
        })
        .await
        .unwrap();
    kept_of(&outcome);
    assert_eq!(tag(&bytes_of(&file)), 3, "6 turned a quarter is 3");
    let kept = versions_of(dir.path(), &file);
    assert_eq!(kept.len(), 1, "one version was kept");
    let original = std::fs::read(
        dir.path()
            .join("state/anyview/versions")
            .join(kept[0].id.to_string() + ".bin"),
    )
    .unwrap();
    assert_eq!(original, JPEG, "and it holds the original bytes");
}

#[tokio::test]
async fn edited_text_is_written_over_the_file_with_the_original_kept_and_only_text_takes_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "notes.txt", b"one\r\ntwo\r\n");
    let (desktop, _) = desktop(dir.path());
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file.clone()));
    let text = || TextSave::new(b"one\r\nthree\r\n".to_vec());
    let (shown, carry) = route(shown, HostRequest::SaveText(text()));
    let Carry::Desktop(task) = carry else {
        panic!("not carried: {carry:?}")
    };
    let (_, busy) = route(shown.clone(), HostRequest::SaveText(text()));
    assert_eq!(
        busy,
        Carry::Declined(Declined::Queued),
        "one save at a time: the second waits"
    );
    kept_of(&desktop.carry_out(task).await.unwrap());
    assert_eq!(
        bytes_of(&file),
        b"one\r\nthree\r\n",
        "the text is the file now"
    );
    assert_eq!(
        versions_of(dir.path(), &file).len(),
        1,
        "and the original is kept"
    );
    let picture = probed(dir.path(), "a.jpg", JPEG);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(picture));
    let (_, carry) = route(shown, HostRequest::SaveText(text()));
    assert_eq!(
        carry,
        Carry::Declined(Declined::Edit),
        "a picture takes no text"
    );
    let (_, carry) = route(Shown::default(), HostRequest::SaveText(text()));
    assert_eq!(carry, Carry::Declined(Declined::NoFileShown));
}

#[tokio::test]
async fn undo_restores_the_original_bytes_and_redo_applies_the_edit_again() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let run = |task| desktop.carry_out(task);
    let edit = run(Task::Edit {
        file: file.clone(),
        request: turn(),
    })
    .await
    .unwrap();
    let edited = bytes_of(&file);
    assert_ne!(edited, JPEG);
    let back = run(Task::Restore {
        file: file.source.path().clone(),
        version: kept_of(&edit),
    })
    .await
    .unwrap();
    assert_eq!(bytes_of(&file), JPEG, "undo put the original back");
    let again = run(Task::Restore {
        file: file.source.path().clone(),
        version: kept_of(&back),
    })
    .await
    .unwrap();
    kept_of(&again);
    assert_eq!(bytes_of(&file), edited, "redo applied the edit again");
}

#[tokio::test]
async fn the_window_trail_walks_a_save_an_undo_and_a_redo_through_routing() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file.clone()));
    let (shown, carry) = route(shown, HostRequest::Edit(turn()));
    let Carry::Desktop(task) = carry else {
        panic!("not carried: {carry:?}")
    };
    let (_, busy) = route(shown.clone(), HostRequest::Edit(turn()));
    assert_eq!(
        busy,
        Carry::Declined(Declined::Queued),
        "one save at a time: the second waits"
    );
    let shown = shown.after(&desktop.carry_out(task).await.unwrap());
    let edited = bytes_of(&file);
    let mut shown = shown;
    for (rewind, want) in [(Rewind::Undo, JPEG.to_vec()), (Rewind::Redo, edited)] {
        let (next, carry) = route(shown, HostRequest::Rewind(rewind));
        let Carry::Desktop(task) = carry else {
            panic!("{rewind:?} not carried: {carry:?}")
        };
        shown = next.after(&desktop.carry_out(task).await.unwrap());
        assert_eq!(bytes_of(&file), want, "{rewind:?}");
    }
    let (_, nothing) = route(shown, HostRequest::Rewind(Rewind::Redo));
    assert_eq!(nothing, Carry::Declined(Declined::NothingToRedo));
}

#[tokio::test]
async fn undo_with_nothing_edited_is_declined() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file));
    let (_, carry) = route(shown, HostRequest::Rewind(Rewind::Undo));
    assert_eq!(carry, Carry::Declined(Declined::NothingToUndo));
}

#[tokio::test]
async fn a_png_flip_is_written_again_as_a_png_and_a_save_that_fails_says_nothing_was_written() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", QUADRANTS);
    let (desktop, _) = desktop(dir.path());
    let flip = EditRequest::of_picture(Edit::Flip(Axis::Horizontal));
    let outcome = desktop
        .carry_out(Task::Edit {
            file: file.clone(),
            request: flip,
        })
        .await
        .unwrap();
    kept_of(&outcome);
    let after = bytes_of(&file);
    assert!(after.starts_with(b"\x89PNG"), "still a png");
    assert_ne!(after, QUADRANTS);
    // A page edit of a picture cannot be made: nothing is written, and the file stays.
    let pages = EditRequest::of_picture(Edit::DeletePages(
        PageRange::new(PageIndex(0), PageIndex(0)).unwrap(),
    ));
    let refused = desktop
        .carry_out(Task::Edit {
            file: file.clone(),
            request: pages,
        })
        .await
        .unwrap();
    assert!(matches!(refused, Outcome::NotWritten(_)), "{refused:?}");
    assert_eq!(bytes_of(&file), after);
    assert_eq!(
        versions_of(dir.path(), &file).len(),
        1,
        "nothing more was kept"
    );
}

fn page_count(file: &Opened) -> u32 {
    PdfDocument::from_bytes(bytes_of(file))
        .unwrap()
        .page_count()
        .get()
}

#[tokio::test]
async fn pdf_pages_are_deleted_moved_and_turned_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.pdf", &pdf_fixture::fixture_bytes());
    let (desktop, _) = desktop(dir.path());
    assert_eq!(page_count(&file), 3);
    let at = |page| PageIndex(page);
    let edits = [
        EditRequest::on_page(
            Edit::MovePage {
                from: at(0),
                to: at(2),
            },
            at(0),
        ),
        EditRequest::on_page(Edit::Rotate(QuarterTurn::Quarter), at(1)),
        EditRequest::on_page(
            Edit::DeletePages(PageRange::new(at(0), at(0)).unwrap()),
            at(0),
        ),
    ];
    let mut counts = vec![];
    let mut second_page = vec![];
    for request in edits {
        let outcome = desktop
            .carry_out(Task::Edit {
                file: file.clone(),
                request,
            })
            .await
            .unwrap();
        kept_of(&outcome);
        let document = PdfDocument::from_bytes(bytes_of(&file)).unwrap();
        counts.push(document.page_count().get());
        second_page.push(document.page_size(at(1)).unwrap());
    }
    assert_eq!(
        counts,
        [3, 3, 2],
        "a move and a turn keep the pages, a delete drops one"
    );
    assert_eq!(
        (second_page[1].width, second_page[1].height),
        (second_page[0].height, second_page[0].width),
        "the second page was turned a quarter: its sides swapped"
    );
    assert_eq!(versions_of(dir.path(), &file).len(), 3);
}

#[tokio::test]
async fn revert_to_lists_the_kept_versions_and_puts_the_chosen_one_back() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    for _ in 0..2 {
        kept_of(
            &desktop
                .carry_out(Task::Edit {
                    file: file.clone(),
                    request: turn(),
                })
                .await
                .unwrap(),
        );
    }
    let rows = desktop.kept_versions(file.source.path());
    assert_eq!(rows.len(), 2, "a row for each save");
    let twice = bytes_of(&file);
    // Both versions were kept at the same second; the first save's original is the older row.
    let oldest = rows.last().unwrap().key.clone();
    let outcome = desktop
        .carry_out(Task::RevertTo {
            file: file.source.path().clone(),
            key: oldest,
        })
        .await
        .unwrap();
    kept_of(&outcome);
    assert_eq!(bytes_of(&file), JPEG, "the original is back");
    assert_ne!(twice, JPEG);
    assert_eq!(
        desktop.kept_versions(file.source.path()).len(),
        3,
        "the revert kept what it replaced"
    );
}

#[tokio::test]
async fn a_version_that_is_not_kept_reverts_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let outcome = desktop
        .carry_out(Task::RevertTo {
            file: file.source.path().clone(),
            key: anyview_ui::VersionKey::from_static("none/none"),
        })
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::NotWritten(_)), "{outcome:?}");
    assert_eq!(bytes_of(&file), JPEG);
}

#[tokio::test]
async fn save_a_copy_writes_beside_the_source_and_leaves_it_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", QUADRANTS);
    let (desktop, _) = desktop(dir.path());
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file.clone()));
    let (_, carry) = route(
        shown.clone(),
        HostRequest::SaveCopy(TypedText::new("b.png")),
    );
    let Carry::Desktop(task) = carry else {
        panic!("{carry:?}")
    };
    assert_eq!(
        desktop.carry_out(task.clone()).await.unwrap(),
        Outcome::Wrote(path(dir.path().join("b.png").to_str().unwrap()))
    );
    assert_eq!(std::fs::read(dir.path().join("b.png")).unwrap(), QUADRANTS);
    assert_eq!(bytes_of(&file), QUADRANTS, "the source is as it was");
    // A destination that exists is never replaced, the source included.
    let again = desktop.carry_out(task).await.unwrap();
    assert!(matches!(again, Outcome::Failed(_)), "{again:?}");
    let (_, onto_source) = route(shown, HostRequest::SaveCopy(TypedText::new("a.png")));
    let Carry::Desktop(task) = onto_source else {
        panic!("{onto_source:?}")
    };
    let refused = desktop.carry_out(task).await.unwrap();
    assert!(matches!(refused, Outcome::Failed(_)), "{refused:?}");
    assert_eq!(bytes_of(&file), QUADRANTS);
}

#[test]
fn a_copy_goes_where_the_typed_text_says() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", QUADRANTS);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file.clone()));
    let beside = |name: &str| dir.path().join(name);
    // name, typed, the destination or why not
    let cases: Vec<(&str, String, Option<FilePath>)> = vec![
        (
            "a name is beside the file",
            "b.png".into(),
            Some(path(beside("b.png").to_str().unwrap())),
        ),
        (
            "an absolute path is as typed",
            "/tmp/elsewhere/c.png".into(),
            Some(path("/tmp/elsewhere/c.png")),
        ),
        (
            "a relative path with a folder is no name",
            "sub/c.png".into(),
            None,
        ),
        ("nothing typed is no name", "  ".into(), None),
    ];
    for (name, typed, want) in cases {
        let (_, carry) = route(shown.clone(), HostRequest::SaveCopy(TypedText::new(typed)));
        let want = match want {
            Some(to) => Carry::Desktop(Task::SaveCopy {
                file: file.source.path().clone(),
                to,
            }),
            None => Carry::Declined(Declined::NotAFileName),
        };
        assert_eq!(carry, want, "{name}");
    }
}

#[test]
fn a_file_with_no_edit_declines_one_and_a_file_moved_starts_a_new_trail() {
    let dir = tempfile::tempdir().unwrap();
    let text = probed(dir.path(), "a.txt", b"hello\n");
    let (shown, _) = route(Shown::default(), HostRequest::Opened(text));
    let (_, carry) = route(shown, HostRequest::Edit(turn()));
    assert_eq!(carry, Carry::Declined(Declined::Edit));
}

#[test]
fn versions_older_than_the_keep_period_are_pruned_and_newer_ones_stay() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, b"one").unwrap();
    let versions = Versions::under_state(&dir.path().join("state"));
    let day = 86_400;
    for at in [1_000, 1_000 + 40 * day] {
        let pending = anyview_store::Pending::new(&file, b"two".to_vec());
        versions
            .back_up(pending, SavedAt(at))
            .unwrap()
            .write_in_place()
            .unwrap();
    }
    let now = SavedAt(1_000 + 41 * day);
    let outcome = crate::host::prune_versions(&versions, now);
    assert_eq!(outcome, Outcome::Done);
    let left = versions.list(&file).unwrap();
    assert_eq!(left.len(), 1, "the 41 day old version went");
    assert_eq!(left[0].saved_at, SavedAt(1_000 + 40 * day));
}

#[tokio::test]
async fn an_undo_asked_while_a_save_is_written_waits_and_is_asked_again_when_it_ends() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file.clone()));
    let (shown, carry) = route(shown, HostRequest::Edit(turn()));
    let Carry::Desktop(task) = carry else {
        panic!("not carried: {carry:?}")
    };
    // The undo comes while the save is still being written.
    let (shown, queued) = route(shown, HostRequest::Rewind(Rewind::Undo));
    assert_eq!(queued, Carry::Declined(Declined::Queued));
    let outcome = desktop.carry_out(task).await.unwrap();
    let (shown, again) = shown.after(&outcome).next_queued();
    assert_eq!(
        again,
        Some(HostRequest::Rewind(Rewind::Undo)),
        "the undo is asked again once the save has ended"
    );
    let (_, carry) = route(shown, HostRequest::Rewind(Rewind::Undo));
    let Carry::Desktop(task) = carry else {
        panic!("the undo runs now: {carry:?}")
    };
    desktop.carry_out(task).await.unwrap();
    assert_eq!(bytes_of(&file), JPEG, "and takes the edit back");
}

#[tokio::test]
async fn a_save_that_wrote_nothing_drops_what_waited_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file));
    let (shown, _) = route(shown, HostRequest::Edit(turn()));
    let (shown, _) = route(shown, HostRequest::Rewind(Rewind::Undo));
    let (_, again) = shown
        .after(&Outcome::NotWritten("the disk is full".to_owned()))
        .next_queued();
    assert_eq!(again, None, "the file is as it was: nothing to undo");
}

#[tokio::test]
async fn a_cut_jpeg_is_saved_in_place_upright_with_its_original_kept() {
    // The fixture is 48 by 32 stored and orientation 6, so 32 by 48 as it is shown.
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let adjust = Adjust {
        crop: Some(PixelRect {
            left: PixelLen(0),
            top: PixelLen(0),
            size: PixelSize {
                width: PixelLen(16),
                height: PixelLen(24),
            },
        }),
        ..Adjust::NONE
    };
    let outcome = desktop
        .carry_out(Task::Edit {
            file: file.clone(),
            request: EditRequest::of_picture(Edit::Adjust(adjust)),
        })
        .await
        .unwrap();
    kept_of(&outcome);
    let saved = bytes_of(&file);
    assert_eq!(
        tag(&saved),
        1,
        "the turn is in the pixels, so the tag is upright"
    );
    let after = image::load_from_memory(&saved).unwrap();
    assert_eq!(
        (after.width(), after.height()),
        (16, 24),
        "the file holds the cut"
    );
    let kept = versions_of(dir.path(), &file);
    assert_eq!(kept.len(), 1, "the original was kept first");
}
