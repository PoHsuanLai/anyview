use super::*;
use crate::testing::settle;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::Shown;
use std::time::Duration;

const REVEAL: Duration = Duration::from_millis(150);
const FADE_IN: ChromeOut = ChromeOut::Fade {
    to: Shown::Visible,
    over: REVEAL,
};
const FADE_OUT: ChromeOut = ChromeOut::Fade {
    to: Shown::Hidden,
    over: REVEAL,
};

const fn stamp(ms: u64) -> Stamp {
    Stamp(ms)
}
const fn revealing(since: u64, until: u64) -> Chrome {
    Chrome::Revealing {
        since: stamp(since),
        until: stamp(until),
    }
}
const fn shown(idle_from: u64, hide_at: u64) -> Chrome {
    Chrome::Shown {
        idle_from: stamp(idle_from),
        hide_at: stamp(hide_at),
    }
}
const fn hiding(since: u64, until: u64) -> Chrome {
    Chrome::Hiding {
        since: stamp(since),
        until: stamp(until),
    }
}
const fn pinned(by: PinReasons) -> Chrome {
    Chrome::Pinned { by }
}

const CAPSULE: PinReasons = PinReasons::of(PinReason::PointerOverCapsule);
const MENU: PinReasons = PinReasons::of(PinReason::MenuOpen);
const PAUSED: PinReasons = PinReasons::of(PinReason::MediaPaused);
const CAPSULE_AND_MENU: PinReasons = CAPSULE.with(PinReason::MenuOpen);

/// Name, state before, input, time, state after, outputs.
type Case = (
    &'static str,
    Chrome,
    ChromeIn,
    u64,
    Chrome,
    &'static [ChromeOut],
);

const CASES: &[Case] = &[
    (
        "pointer over content reveals a hidden chrome",
        Chrome::Hidden,
        ChromeIn::PointerMoved(Zone::Content),
        1000,
        revealing(1000, 1150),
        &[FADE_IN],
    ),
    (
        "a pin reveals a hidden chrome and holds it",
        Chrome::Hidden,
        ChromeIn::Pin(PinReason::MenuOpen),
        1000,
        pinned(MENU),
        &[FADE_IN],
    ),
    (
        "leaving does nothing to a hidden chrome",
        Chrome::Hidden,
        ChromeIn::PointerLeft,
        1000,
        Chrome::Hidden,
        &[],
    ),
    (
        "the reveal ends at its deadline and idle counting starts",
        revealing(1000, 1150),
        ChromeIn::Elapsed,
        1150,
        shown(1150, 3150),
        &[],
    ),
    (
        "an early wake keeps revealing",
        revealing(1000, 1150),
        ChromeIn::Elapsed,
        1149,
        revealing(1000, 1150),
        &[],
    ),
    (
        "a pin during the reveal holds without another fade",
        revealing(1000, 1150),
        ChromeIn::Pin(PinReason::MediaPaused),
        1100,
        pinned(PAUSED),
        &[],
    ),
    (
        "moving over content restarts the idle count",
        shown(1150, 3150),
        ChromeIn::PointerMoved(Zone::Content),
        2000,
        shown(2000, 4000),
        &[],
    ),
    (
        "moving onto the capsule pins it",
        shown(1150, 3150),
        ChromeIn::PointerMoved(Zone::Capsule),
        2000,
        pinned(CAPSULE),
        &[],
    ),
    (
        "leaving the window does not move the idle count",
        shown(1150, 3150),
        ChromeIn::PointerLeft,
        2000,
        shown(1150, 3150),
        &[],
    ),
    (
        "an early wake keeps it shown",
        shown(1150, 3150),
        ChromeIn::Elapsed,
        3149,
        shown(1150, 3150),
        &[],
    ),
    (
        "it hides when the idle deadline comes",
        shown(1150, 3150),
        ChromeIn::Elapsed,
        3150,
        hiding(3150, 3300),
        &[FADE_OUT],
    ),
    (
        "a second pin joins the first",
        pinned(CAPSULE),
        ChromeIn::Pin(PinReason::MenuOpen),
        2000,
        pinned(CAPSULE_AND_MENU),
        &[],
    ),
    (
        "unpinning one of two keeps it pinned",
        pinned(CAPSULE_AND_MENU),
        ChromeIn::Unpin(PinReason::PointerOverCapsule),
        2000,
        pinned(MENU),
        &[],
    ),
    (
        "unpinning the last reason starts the idle count from then",
        pinned(MENU),
        ChromeIn::Unpin(PinReason::MenuOpen),
        2000,
        shown(2000, 4000),
        &[],
    ),
    (
        "unpinning a reason that is not held changes nothing",
        pinned(MENU),
        ChromeIn::Unpin(PinReason::KeyboardFocus),
        2000,
        pinned(MENU),
        &[],
    ),
    (
        "leaving drops only the capsule reason",
        pinned(CAPSULE_AND_MENU),
        ChromeIn::PointerLeft,
        2000,
        pinned(MENU),
        &[],
    ),
    (
        "moving over content while paused keeps the pause pin",
        pinned(PAUSED),
        ChromeIn::PointerMoved(Zone::Content),
        2000,
        pinned(PAUSED),
        &[],
    ),
    (
        "a pinned chrome ignores the clock",
        pinned(PAUSED),
        ChromeIn::Elapsed,
        9000,
        pinned(PAUSED),
        &[],
    ),
    (
        "pointer over content during the hide fades back in",
        hiding(3150, 3300),
        ChromeIn::PointerMoved(Zone::Content),
        3200,
        revealing(3200, 3350),
        &[FADE_IN],
    ),
    (
        "a pin during the hide fades back in and holds",
        hiding(3150, 3300),
        ChromeIn::Pin(PinReason::KeyboardFocus),
        3200,
        pinned(PinReasons::of(PinReason::KeyboardFocus)),
        &[FADE_IN],
    ),
    (
        "the hide ends at its deadline",
        hiding(3150, 3300),
        ChromeIn::Elapsed,
        3300,
        Chrome::Hidden,
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    let params = ChromeParams::default();
    for (name, from, input, at, state, outs) in CASES {
        let (next, out) = from.step(*input, stamp(*at), &params, &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
    }
}

#[test]
fn wake_is_set_only_while_a_timer_runs() {
    // name, state, wake
    const WAKES: &[(&str, Chrome, Option<Stamp>)] = &[
        ("hidden", Chrome::Hidden, None),
        ("revealing", revealing(0, 150), Some(stamp(150))),
        ("shown", shown(150, 2150), Some(stamp(2150))),
        ("pinned", pinned(MENU), None),
        ("hiding", hiding(2150, 2300), Some(stamp(2300))),
    ];
    for (name, state, wake) in WAKES {
        assert_eq!(state.wake(), *wake, "{name}");
    }
}

#[test]
fn the_chrome_hides_exactly_at_idle_from_plus_hide_after() {
    let params = ChromeParams::default();
    let (revealing, _) = Chrome::Hidden.step(
        ChromeIn::PointerMoved(Zone::Content),
        stamp(5000),
        &params,
        &(),
    );
    let (rest, log) = settle(revealing, &params, &(), 10);
    assert_eq!(rest, Chrome::Hidden);
    // Revealed at 5150 (the fade), idle from then, hidden at 5150 + 2000, fully gone 150 later.
    assert_eq!(log, vec![(stamp(7150), FADE_OUT)]);
}

#[test]
fn a_move_before_the_deadline_pushes_the_hide_out_by_the_whole_delay() {
    let params = ChromeParams::default();
    let (state, _) = Chrome::Hidden.step(
        ChromeIn::PointerMoved(Zone::Content),
        stamp(0),
        &params,
        &(),
    );
    let (state, _) = state.step(ChromeIn::Elapsed, stamp(150), &params, &());
    assert_eq!(state.wake(), Some(stamp(2150)));
    let (state, _) = state.step(
        ChromeIn::PointerMoved(Zone::Content),
        stamp(2100),
        &params,
        &(),
    );
    assert_eq!(state.wake(), Some(stamp(4100)));
    let (rest, log) = settle(state, &params, &(), 10);
    assert_eq!(rest, Chrome::Hidden);
    assert_eq!(log, vec![(stamp(4100), FADE_OUT)]);
}

#[test]
fn settings_that_change_apply_from_the_next_step() {
    let slow = ChromeParams {
        hide_after: Duration::from_millis(5000),
        ..ChromeParams::default()
    };
    let (state, _) =
        Chrome::Hidden.step(ChromeIn::PointerMoved(Zone::Content), stamp(0), &slow, &());
    let (state, _) = state.step(ChromeIn::Elapsed, stamp(150), &slow, &());
    assert_eq!(state, shown(150, 5150));
}

#[test]
fn pin_reasons_stack_and_release_one_at_a_time() {
    let both = PinReasons::of(PinReason::MenuOpen).with(PinReason::MediaPaused);
    assert_eq!(
        both.iter().collect::<Vec<_>>(),
        vec![PinReason::MenuOpen, PinReason::MediaPaused]
    );
    let one = both.without(PinReason::MenuOpen);
    assert_eq!(one, Some(PAUSED));
    assert_eq!(
        one.and_then(|set| set.without(PinReason::MediaPaused)),
        None
    );
    assert!(both.contains(PinReason::MenuOpen));
    assert!(!PAUSED.contains(PinReason::MenuOpen));
}
