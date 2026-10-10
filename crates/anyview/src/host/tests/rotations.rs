//! Turning and flipping a picture in place, again and again: the turns that cancel leave the
//! picture as it was.

use super::support::{desktop, probed};
use crate::host::{Hosting, Outcome, Task};
use anyview_core::{Axis, Edit, QuarterTurn};
use anyview_image::ExifFacts;
use anyview_ui::{EditRequest, Opened};

const JPEG: &[u8] = include_bytes!("../../../../anyview-image/tests/fixtures/rotated.jpg");
const QUADRANTS: &[u8] = include_bytes!("../../../../anyview-image/tests/fixtures/quadrants.png");

fn pixels(file: &Opened) -> image::RgbaImage {
    image::open(file.source.path().as_path())
        .unwrap()
        .to_rgba8()
}

async fn apply(desktop: &super::support::TestDesktop, file: &Opened, edits: &[Edit]) {
    for edit in edits {
        let outcome = desktop
            .carry_out(Task::Edit {
                file: file.clone(),
                request: EditRequest::of_picture(*edit),
            })
            .await
            .unwrap();
        assert!(
            matches!(outcome, Outcome::Written { .. }),
            "{edit:?}: {outcome:?}"
        );
    }
}

#[tokio::test]
async fn four_quarter_turns_of_a_png_give_back_the_same_pixels() {
    for turn in [QuarterTurn::Quarter, QuarterTurn::ThreeQuarter] {
        let dir = tempfile::tempdir().unwrap();
        let file = probed(dir.path(), "a.png", QUADRANTS);
        let before = pixels(&file);
        let (desktop, _) = desktop(dir.path());
        apply(&desktop, &file, &[Edit::Rotate(turn); 4]).await;
        assert_eq!(pixels(&file), before, "{turn:?} four times");
    }
}

#[tokio::test]
async fn a_turn_right_then_left_and_two_flips_give_back_the_same_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", QUADRANTS);
    let before = pixels(&file);
    let (desktop, _) = desktop(dir.path());
    apply(
        &desktop,
        &file,
        &[
            Edit::Rotate(QuarterTurn::Quarter),
            Edit::Rotate(QuarterTurn::ThreeQuarter),
            Edit::Flip(Axis::Horizontal),
            Edit::Flip(Axis::Horizontal),
            Edit::Flip(Axis::Vertical),
            Edit::Flip(Axis::Vertical),
        ],
    )
    .await;
    assert_eq!(pixels(&file), before);
}

#[tokio::test]
async fn four_quarter_turns_of_a_jpeg_give_back_its_orientation() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.jpg", JPEG);
    let (desktop, _) = desktop(dir.path());
    let before = ExifFacts::read(JPEG).orientation.tag();
    apply(&desktop, &file, &[Edit::Rotate(QuarterTurn::Quarter); 4]).await;
    let bytes = std::fs::read(file.source.path().as_path()).unwrap();
    assert_eq!(ExifFacts::read(&bytes).orientation.tag(), before);
}
