use super::{MIN_KEPT, MediaTrim, TrimIn, TrimWindow};
use crate::stage::TrimEdge::{End, Start};
use anyview_core::{Edit, MediaLength, MediaTime};

const fn secs(seconds: u64) -> MediaTime {
    MediaTime::from_secs(seconds)
}

const WHOLE: MediaLength = MediaLength(secs(60));

const fn window(start: MediaTime, end: MediaTime) -> TrimWindow {
    TrimWindow { start, end }
}

fn run(inputs: &[TrimIn]) -> MediaTrim {
    inputs
        .iter()
        .fold(MediaTrim::default(), |trim, input| trim.step(*input))
}

fn up() -> TrimIn {
    TrimIn::Begin(WHOLE)
}

const fn moved(edge: crate::stage::TrimEdge, to: MediaTime) -> TrimIn {
    TrimIn::Move { edge, to }
}

#[test]
fn the_bar_comes_up_on_the_whole_recording_and_not_on_one_too_short_to_keep_a_part_of() {
    let trim = run(&[up()]);
    assert_eq!(trim.handles(), Some((WHOLE, window(secs(0), secs(60)))));
    assert!(trim.is_choosing());
    assert!(!trim.is_edited(), "nothing is trimmed by looking");
    assert_eq!(
        run(&[TrimIn::Begin(MediaLength(MIN_KEPT))]),
        MediaTrim::default()
    );
}

#[test]
fn a_handle_stays_inside_the_recording_and_short_of_the_other_by_the_least_kept() {
    let short = |end: MediaTime| MediaTime(end.0 - MIN_KEPT.0);
    let after = |start: MediaTime| MediaTime(start.0 + MIN_KEPT.0);
    // name, what was done, the stretch between the handles
    let cases = [
        (
            "the start moves",
            vec![moved(Start, secs(10))],
            window(secs(10), secs(60)),
        ),
        (
            "the end moves",
            vec![moved(End, secs(40))],
            window(secs(0), secs(40)),
        ),
        (
            "the start stops short of the end",
            vec![moved(Start, secs(90))],
            window(short(secs(60)), secs(60)),
        ),
        (
            "the end stops short of the start by the same",
            vec![moved(Start, secs(20)), moved(End, secs(5))],
            window(secs(20), after(secs(20))),
        ),
        (
            "the end stops at the end of the recording",
            vec![moved(End, secs(500))],
            window(secs(0), secs(60)),
        ),
        (
            "the start stops where the moved end leaves room",
            vec![moved(End, secs(30)), moved(Start, secs(30))],
            window(short(secs(30)), secs(30)),
        ),
        (
            "a handle can be let back out",
            vec![moved(Start, secs(20)), moved(Start, secs(0))],
            window(secs(0), secs(60)),
        ),
    ];
    for (name, moves, want) in cases {
        let trim = moves
            .into_iter()
            .fold(run(&[up()]), |trim, input| trim.step(input));
        assert_eq!(trim.handles().map(|(_, now)| now), Some(want), "{name}");
    }
}

#[test]
fn a_handle_moved_with_no_bar_up_moves_nothing() {
    assert_eq!(run(&[moved(Start, secs(5))]), MediaTrim::default());
    let applied = run(&[up(), moved(Start, secs(5)), TrimIn::Apply]);
    assert_eq!(applied.clone().step(moved(End, secs(9))), applied);
}

#[test]
fn what_plays_is_the_stretch_between_the_handles_and_then_the_one_applied() {
    let cases = [
        ("nothing yet", vec![], None),
        (
            "the bar is up: the whole recording",
            vec![up()],
            Some(window(secs(0), secs(60))),
        ),
        (
            "the handles move",
            vec![up(), moved(Start, secs(10)), moved(End, secs(50))],
            Some(window(secs(10), secs(50))),
        ),
        (
            "applied",
            vec![
                up(),
                moved(Start, secs(10)),
                moved(End, secs(50)),
                TrimIn::Apply,
            ],
            Some(window(secs(10), secs(50))),
        ),
        (
            "the bar up again over a trim plays what the handles say",
            vec![
                up(),
                moved(Start, secs(10)),
                TrimIn::Apply,
                up(),
                moved(Start, secs(30)),
            ],
            Some(window(secs(30), secs(60))),
        ),
        (
            "cancelling that leaves the applied trim playing",
            vec![
                up(),
                moved(Start, secs(10)),
                TrimIn::Apply,
                up(),
                moved(Start, secs(30)),
                TrimIn::Cancel,
            ],
            Some(window(secs(10), secs(60))),
        ),
    ];
    for (name, inputs, want) in cases {
        assert_eq!(run(&inputs).play_range(), want, "{name}");
    }
}

#[test]
fn applying_keeps_the_stretch_and_saving_it_is_an_edit_of_that_stretch() {
    let trim = run(&[
        up(),
        moved(Start, secs(10)),
        moved(End, secs(50)),
        TrimIn::Apply,
    ]);
    assert!(!trim.is_choosing());
    assert!(trim.is_edited());
    assert_eq!(trim.applied(), Some(window(secs(10), secs(50))));
    assert_eq!(
        trim.applied().map(|kept| kept.length()),
        Some(MediaLength(secs(40)))
    );
    assert_eq!(
        trim.edit(),
        Some(Edit::Trim {
            start: secs(10),
            end: secs(50)
        })
    );
    // Without anything chosen, nothing is a trim.
    let whole = run(&[up(), TrimIn::Apply]);
    assert!(!whole.is_edited());
    assert_eq!(whole.edit(), None);
    // Widening a trim back to everything undoes it.
    let widened = trim
        .step(up())
        .step(moved(Start, secs(0)))
        .step(moved(End, secs(60)));
    assert!(!widened.step(TrimIn::Apply).is_edited());
}

#[test]
fn cancelling_leaves_the_trim_as_it_was() {
    let cases = [
        (
            "none before",
            vec![up(), moved(Start, secs(10)), TrimIn::Cancel],
            None,
        ),
        (
            "one before",
            vec![
                up(),
                moved(Start, secs(10)),
                TrimIn::Apply,
                up(),
                moved(Start, secs(40)),
                TrimIn::Cancel,
            ],
            Some(window(secs(10), secs(60))),
        ),
    ];
    for (name, inputs, want) in cases {
        let trim = run(&inputs);
        assert!(!trim.is_choosing(), "{name}");
        assert_eq!(trim.applied(), want, "{name}");
    }
}

#[test]
fn an_applied_trim_is_taken_back_and_done_again_until_another_replaces_it() {
    let applied = run(&[up(), moved(Start, secs(10)), TrimIn::Apply]);
    assert!(applied.can_undo() && !applied.can_redo());

    let undone = applied.clone().step(TrimIn::Undo);
    assert!(!undone.is_edited() && undone.can_redo() && !undone.can_undo());
    assert_eq!(undone.play_range(), None, "the whole recording plays again");

    assert_eq!(undone.clone().step(TrimIn::Redo), applied);

    let replaced = undone
        .step(up())
        .step(moved(End, secs(20)))
        .step(TrimIn::Apply);
    assert!(!replaced.can_redo(), "a new trim ends the old redo");

    // Taking back waits for the bar to be put away.
    let choosing = applied.clone().step(up());
    assert_eq!(choosing.clone().step(TrimIn::Undo), choosing);
    // A recording that is nothing to undo.
    assert_eq!(
        MediaTrim::default().step(TrimIn::Undo),
        MediaTrim::default()
    );
    assert_eq!(
        MediaTrim::default().step(TrimIn::Redo),
        MediaTrim::default()
    );
    assert_eq!(applied.step(TrimIn::Discard), MediaTrim::default());
}
