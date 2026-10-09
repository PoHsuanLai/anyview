use super::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

enum Act {
    Nothing,
    Request,
}

/// (name, has a deadline this far after the base instant, action, asked at this far after the
/// base instant, expected state)
const CASES: &[(&str, Option<u64>, Act, u64, StopState)] = &[
    ("fresh stop runs", None, Act::Nothing, 0, StopState::Running),
    (
        "no deadline never lapses",
        None,
        Act::Nothing,
        1_000_000,
        StopState::Running,
    ),
    ("request stops", None, Act::Request, 0, StopState::Stopped),
    (
        "before the deadline runs",
        Some(100),
        Act::Nothing,
        99,
        StopState::Running,
    ),
    (
        "at the deadline stops",
        Some(100),
        Act::Nothing,
        100,
        StopState::Stopped,
    ),
    (
        "past the deadline stops",
        Some(100),
        Act::Nothing,
        101,
        StopState::Stopped,
    ),
    (
        "request before the deadline stops",
        Some(100),
        Act::Request,
        1,
        StopState::Stopped,
    ),
];

#[test]
fn stopped_at_follows_the_flag_and_the_deadline() {
    let base = Instant::now();
    for (name, deadline, act, at, expected) in CASES {
        let stop = match deadline {
            Some(ms) => Stop::with_deadline(base + Duration::from_millis(*ms)),
            None => Stop::new(),
        };
        match act {
            Act::Nothing => {}
            Act::Request => stop.request(),
        }
        let got = stop.stopped_at(base + Duration::from_millis(*at));
        assert_eq!(got, *expected, "{name}");
    }
}

#[test]
fn stopped_ignores_the_deadline() {
    let base = Instant::now();
    let stop = Stop::with_deadline(base);
    assert_eq!(stop.stopped(), StopState::Running);
    assert_eq!(stop.stopped_at(base), StopState::Stopped);
    assert_eq!(stop.deadline(), Some(base));
}

#[test]
fn the_flag_is_the_one_every_clone_shares() {
    let stop = Stop::new();
    let flag = stop.flag();
    let worker_side = stop.clone();
    assert!(!flag.load(Ordering::Acquire));
    stop.request();
    assert!(flag.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(&flag, &worker_side.flag()));
    // Raising the flag from outside is a request too.
    let other = Stop::new();
    other.flag().store(true, Ordering::Release);
    assert_eq!(other.stopped(), StopState::Stopped);
}

#[test]
fn tickets_count_up_and_saturate() {
    const CASES: &[(Ticket, Ticket)] = &[
        (Ticket(0), Ticket(1)),
        (Ticket(41), Ticket(42)),
        (Ticket(u64::MAX), Ticket(u64::MAX)),
    ];
    for (from, next) in CASES {
        assert_eq!(from.next(), *next);
    }
}

#[test]
fn ticketed_is_current_only_for_its_own_ticket() {
    const CASES: &[(Ticket, Ticket, bool)] = &[
        (Ticket(3), Ticket(3), true),
        (Ticket(2), Ticket(3), false),
        (Ticket(4), Ticket(3), false),
    ];
    for (carried, current, expected) in CASES {
        let result = Ticketed::new(*carried, "pixels");
        assert_eq!(
            result.is_current(*current),
            *expected,
            "{carried:?} vs {current:?}"
        );
    }
}

#[test]
fn ticketed_map_keeps_the_ticket() {
    let mapped = Ticketed::new(Ticket(7), 20).map(|n| n + 1);
    assert_eq!(mapped, Ticketed::new(Ticket(7), 21));
}
