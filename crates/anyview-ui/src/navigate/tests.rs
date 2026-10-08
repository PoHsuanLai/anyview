use super::*;
use anyview_core::{FilePath, Heading, Neighbours, NonEmpty, Sequence, SequenceOrigin};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::ShortcutKey;

const FILES: &[&str] = &["/a.png", "/b.png", "/c.png"];

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

fn walking_at(files: &[&str], index: usize) -> Navigate {
    let entries: Vec<FilePath> = files.iter().map(|file| path(file)).collect();
    let entries = NonEmpty::from_vec(entries).unwrap();
    let sequence = Sequence::starting_at(
        entries.clone(),
        &path(files[index]),
        SequenceOrigin::Selection,
    )
    .unwrap();
    Navigate::Walking {
        sequence,
        heading: Heading::Onward,
    }
}

fn position(state: &Navigate) -> Option<usize> {
    match state {
        Navigate::Idle => None,
        Navigate::Walking { sequence, .. } => Some(sequence.at().index()),
    }
}

/// Name, files, position before, input, position after, file opened, preloaded (before, after).
type Case = (
    &'static str,
    &'static [&'static str],
    usize,
    fn() -> NavigateIn,
    usize,
    Option<&'static str>,
    Option<(Option<&'static str>, Option<&'static str>)>,
);

const CASES: &[Case] = &[
    (
        "next opens the following file",
        FILES,
        0,
        || NavigateIn::Next,
        1,
        Some("/b.png"),
        Some((Some("/a.png"), Some("/c.png"))),
    ),
    (
        "next at the end stops",
        FILES,
        2,
        || NavigateIn::Next,
        2,
        None,
        None,
    ),
    (
        "previous opens the preceding file",
        FILES,
        2,
        || NavigateIn::Previous,
        1,
        Some("/b.png"),
        Some((Some("/a.png"), Some("/c.png"))),
    ),
    (
        "previous at the start stops",
        FILES,
        0,
        || NavigateIn::Previous,
        0,
        None,
        None,
    ),
    (
        "first jumps to the start",
        FILES,
        2,
        || NavigateIn::First,
        0,
        Some("/a.png"),
        Some((None, Some("/b.png"))),
    ),
    (
        "last jumps to the end",
        FILES,
        0,
        || NavigateIn::Last,
        2,
        Some("/c.png"),
        Some((Some("/b.png"), None)),
    ),
    (
        "first on the first file opens nothing",
        FILES,
        0,
        || NavigateIn::First,
        0,
        None,
        None,
    ),
    (
        "a one-file list never moves",
        &["/only.png"],
        0,
        || NavigateIn::Next,
        0,
        None,
        None,
    ),
    (
        "the clock does nothing",
        FILES,
        1,
        || NavigateIn::Elapsed,
        1,
        None,
        None,
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, files, from, input, to, opened, preloaded) in CASES {
        let (next, outs) = walking_at(files, *from).step(input(), Stamp(0), &(), &());
        assert_eq!(position(&next), Some(*to), "{name}: position");
        let want: Vec<NavigateOut> = opened
            .iter()
            .map(|file| NavigateOut::Open(path(file)))
            .chain(preloaded.iter().map(|(previous, next)| {
                NavigateOut::Preload(Neighbours {
                    previous: previous.map(path),
                    next: next.map(path),
                })
            }))
            .collect();
        assert_eq!(outs, want, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn a_walk_starts_from_the_open_file_and_preloads_around_it() {
    let Navigate::Walking { sequence, .. } = walking_at(FILES, 1) else {
        panic!("walking_at builds a walk");
    };
    let (state, outs) = Navigate::Idle.step(NavigateIn::Start(sequence), Stamp(0), &(), &());
    assert_eq!(position(&state), Some(1));
    assert_eq!(
        outs,
        vec![NavigateOut::Preload(Neighbours {
            previous: Some(path("/a.png")),
            next: Some(path("/c.png")),
        })]
    );
}

#[test]
fn moves_with_no_list_do_nothing() {
    let (state, outs) = Navigate::Idle.step(NavigateIn::Next, Stamp(0), &(), &());
    assert_eq!((state, outs), (Navigate::Idle, vec![]));
}

#[test]
fn arrows_and_home_end_walk() {
    // name, key, input
    const KEYS: &[(&str, ShortcutKey, Option<NavigateIn>)] = &[
        ("right", ShortcutKey::Right, Some(NavigateIn::Next)),
        ("left", ShortcutKey::Left, Some(NavigateIn::Previous)),
        ("home", ShortcutKey::Home, Some(NavigateIn::First)),
        ("end", ShortcutKey::End, Some(NavigateIn::Last)),
        ("up", ShortcutKey::Up, None),
    ];
    for (name, key, want) in KEYS {
        assert_eq!(NavigateIn::from_key(&[*key]), *want, "{name}");
    }
}

#[test]
fn leaving_ends_the_walk_and_a_new_list_starts_another() {
    let (state, outs) = walking_at(FILES, 1).step(NavigateIn::Leave, Stamp(0), &(), &());
    assert_eq!((&state, &outs), (&Navigate::Idle, &vec![]), "a walk ends");
    let (state, outs) = state.step(NavigateIn::Next, Stamp(0), &(), &());
    assert_eq!(
        (state, outs),
        (Navigate::Idle, vec![]),
        "and arrows go nowhere"
    );
    let (state, outs) = Navigate::Idle.step(NavigateIn::Leave, Stamp(0), &(), &());
    assert_eq!(
        (state, outs),
        (Navigate::Idle, vec![]),
        "no list to leave is no change"
    );
}

#[test]
fn a_file_that_has_gone_leaves_the_walk_and_the_one_the_person_was_heading_for_opens() {
    // name, the key that brought the walk here, the file it is on, the file then opened
    const CASES: &[(&str, NavigateIn, usize, &str)] = &[
        ("an arrow right goes on", NavigateIn::Next, 1, "/c.png"),
        ("an arrow left goes on", NavigateIn::Previous, 1, "/a.png"),
        (
            "end falls back to the one before",
            NavigateIn::Last,
            2,
            "/b.png",
        ),
        ("home goes to the one after", NavigateIn::First, 0, "/b.png"),
    ];
    for (name, arrived, at, want) in CASES {
        // The walk is one before `at` in the direction of the move, so the move lands on `at`.
        let from = match arrived {
            NavigateIn::Next => at - 1,
            NavigateIn::Previous => at + 1,
            _ => 1,
        };
        let (walk, _) = walking_at(FILES, from).step(arrived.clone(), Stamp(0), &(), &());
        let (walk, outs) = walk.step(NavigateIn::Gone, Stamp(0), &(), &());
        assert!(
            outs.contains(&NavigateOut::Open(path(want))),
            "{name}: {outs:?}"
        );
        let Navigate::Walking { sequence, .. } = walk else {
            panic!("{name}: still walking");
        };
        assert_eq!(sequence.entries().count().get(), FILES.len() - 1, "{name}");
        assert!(
            sequence
                .entries()
                .iter()
                .all(|entry| *entry != path(FILES[*at])),
            "{name}: the gone file left the list"
        );
    }
}

#[test]
fn the_only_file_left_stays_when_it_is_gone() {
    let walk = walking_at(&["/a.png"], 0);
    let (after, outs) = walk.clone().step(NavigateIn::Gone, Stamp(0), &(), &());
    assert_eq!((after, outs), (walk, vec![]));
}
