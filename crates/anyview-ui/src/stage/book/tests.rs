use super::*;
use anyview_core::{LineIndex, Resume, SectionCount, SectionIndex};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const fn at(section: u32) -> BookStage {
    BookStage::Reading {
        section: SectionIndex(section),
    }
}

const fn remember(section: u32) -> BookOut {
    BookOut::Remember(Resume::Book {
        section: SectionIndex(section),
    })
}

/// Name, sections in the book, state before, input, state after, outputs.
type Case = (
    &'static str,
    u32,
    BookStage,
    BookIn,
    BookStage,
    &'static [BookOut],
);

const CASES: &[Case] = &[
    (
        "next moves on and is remembered",
        5,
        at(1),
        BookIn::Next,
        at(2),
        &[remember(2)],
    ),
    (
        "next at the last section stays",
        5,
        at(4),
        BookIn::Next,
        at(4),
        &[],
    ),
    (
        "previous moves back",
        5,
        at(3),
        BookIn::Previous,
        at(2),
        &[remember(2)],
    ),
    (
        "previous at the first section stays",
        5,
        at(0),
        BookIn::Previous,
        at(0),
        &[],
    ),
    (
        "first goes to the start",
        5,
        at(3),
        BookIn::First,
        at(0),
        &[remember(0)],
    ),
    (
        "last goes to the end",
        5,
        at(1),
        BookIn::Last,
        at(4),
        &[remember(4)],
    ),
    (
        "go to a section",
        5,
        at(0),
        BookIn::GoTo(SectionIndex(3)),
        at(3),
        &[remember(3)],
    ),
    (
        "go to past the end lands on the last",
        5,
        at(0),
        BookIn::GoTo(SectionIndex(40)),
        at(4),
        &[remember(4)],
    ),
    (
        "go to where it is says nothing",
        5,
        at(2),
        BookIn::GoTo(SectionIndex(2)),
        at(2),
        &[],
    ),
    (
        "restoring puts the place back without asking to remember it",
        5,
        at(0),
        BookIn::Restore(Resume::Book {
            section: SectionIndex(3),
        }),
        at(3),
        &[],
    ),
    (
        "a restored place past the end is the last section",
        5,
        at(0),
        BookIn::Restore(Resume::Book {
            section: SectionIndex(99),
        }),
        at(4),
        &[],
    ),
    (
        "another kind of place is refused",
        5,
        at(2),
        BookIn::Restore(Resume::Text { line: LineIndex(9) }),
        at(2),
        &[],
    ),
];

#[test]
fn every_case_of_the_table_steps_as_written() {
    for (name, sections, before, input, after, outs) in CASES {
        let params = BookParams {
            sections: SectionCount::new(*sections).unwrap(),
        };
        let (state, out) = before.step(input.clone(), Stamp(0), &params, &());
        assert_eq!(state, *after, "{name}: state");
        assert_eq!(out, *outs, "{name}: outputs");
    }
}

#[test]
fn the_stage_never_asks_to_be_woken() {
    assert_eq!(at(2).wake(), None);
}
