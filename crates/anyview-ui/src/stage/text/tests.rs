use super::super::find::{FindHits, FindOut, HitCount, HitIndex};
use super::*;
use crate::typed::TypedText;
use anyview_core::{LineIndex, Resume};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const CAT: TypedText = TypedText::from_static("cat");
const DOG: TypedText = TypedText::from_static("dog");

const fn place(line: u32, wrap: Wrap, view: TextView) -> TextPlace {
    TextPlace {
        line: LineIndex(line),
        wrap,
        view,
    }
}
const fn reading(line: u32, wrap: Wrap, view: TextView) -> TextStage {
    TextStage::Reading {
        place: place(line, wrap, view),
    }
}
const fn finding(hits: FindHits, line: u32) -> TextStage {
    TextStage::Finding {
        query: CAT,
        hits,
        place: place(line, Wrap::On, TextView::Rendered),
    }
}
const fn found(count: u32, current: u32) -> FindHits {
    FindHits::answered(HitCount(count), HitIndex(current))
}
const fn show_hit(hit: u32) -> TextOut {
    TextOut::Find(FindOut::ShowHit(HitIndex(hit)))
}

use TextView::{Rendered, Source};
use Wrap::{Off, On};

/// Name, the file's views, state before, input, state after, outputs.
type Case = (
    &'static str,
    TextViews,
    TextStage,
    TextIn,
    TextStage,
    &'static [TextOut],
);

const BOTH: TextViews = TextViews::RenderedAndSource;
const SOURCE_ONLY: TextViews = TextViews::SourceOnly;

const CASES: &[Case] = &[
    (
        "scrolling moves the line and is remembered",
        BOTH,
        reading(0, On, Rendered),
        TextIn::Scroll(LineIndex(40)),
        reading(40, On, Rendered),
        &[TextOut::Remember(Resume::Text {
            line: LineIndex(40),
        })],
    ),
    (
        "toggling source switches a Markdown file to its source",
        BOTH,
        reading(5, On, Rendered),
        TextIn::ToggleSource,
        reading(5, On, Source),
        &[TextOut::Show(Source)],
    ),
    (
        "toggling again switches back",
        BOTH,
        reading(5, On, Source),
        TextIn::ToggleSource,
        reading(5, On, Rendered),
        &[TextOut::Show(Rendered)],
    ),
    (
        "a file with only source has nothing to toggle",
        SOURCE_ONLY,
        reading(5, On, Source),
        TextIn::ToggleSource,
        reading(5, On, Source),
        &[],
    ),
    (
        "toggling wrap flips it",
        BOTH,
        reading(5, On, Rendered),
        TextIn::ToggleWrap,
        reading(5, Off, Rendered),
        &[],
    ),
    (
        "toggling wrap flips it back",
        BOTH,
        reading(5, Off, Rendered),
        TextIn::ToggleWrap,
        reading(5, On, Rendered),
        &[],
    ),
    (
        "find starts a search",
        BOTH,
        reading(5, On, Rendered),
        TextIn::Find(CAT),
        finding(FindHits::Pending, 5),
        &[TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "an empty find is ignored",
        BOTH,
        reading(5, On, Rendered),
        TextIn::Find(TypedText::EMPTY),
        reading(5, On, Rendered),
        &[],
    ),
    (
        "restoring a stored line scrolls to it",
        BOTH,
        reading(0, On, Rendered),
        TextIn::Restore(Resume::Text {
            line: LineIndex(480),
        }),
        reading(480, On, Rendered),
        &[TextOut::ScrollTo(LineIndex(480))],
    ),
    (
        "restoring another format's place is ignored",
        BOTH,
        reading(0, On, Rendered),
        TextIn::Restore(Resume::Nothing),
        reading(0, On, Rendered),
        &[],
    ),
    (
        "the hits arrive and the nearest is shown",
        BOTH,
        finding(FindHits::Pending, 0),
        TextIn::Results {
            query: CAT,
            count: HitCount(3),
            nearest: HitIndex(1),
        },
        finding(found(3, 1), 0),
        &[show_hit(1)],
    ),
    (
        "hits for an earlier query are ignored",
        BOTH,
        finding(FindHits::Pending, 0),
        TextIn::Results {
            query: DOG,
            count: HitCount(3),
            nearest: HitIndex(1),
        },
        finding(FindHits::Pending, 0),
        &[],
    ),
    (
        "a search with no hits says so",
        BOTH,
        finding(FindHits::Pending, 0),
        TextIn::Results {
            query: CAT,
            count: HitCount(0),
            nearest: HitIndex(0),
        },
        finding(FindHits::NoMatch, 0),
        &[],
    ),
    (
        "next hit after the last wraps to the first",
        BOTH,
        finding(found(3, 2), 0),
        TextIn::NextHit,
        finding(found(3, 0), 0),
        &[show_hit(0)],
    ),
    (
        "previous hit before the first wraps to the last",
        BOTH,
        finding(found(3, 0), 0),
        TextIn::PreviousHit,
        finding(found(3, 2), 0),
        &[show_hit(2)],
    ),
    (
        "next hit with no matches is nothing",
        BOTH,
        finding(FindHits::NoMatch, 0),
        TextIn::NextHit,
        finding(FindHits::NoMatch, 0),
        &[],
    ),
    (
        "closing the find returns to reading",
        BOTH,
        finding(found(3, 1), 7),
        TextIn::CloseFind,
        reading(7, On, Rendered),
        &[TextOut::Find(FindOut::Clear)],
    ),
    (
        "an empty query closes the find",
        BOTH,
        finding(found(3, 1), 7),
        TextIn::Find(TypedText::EMPTY),
        reading(7, On, Rendered),
        &[TextOut::Find(FindOut::Clear)],
    ),
    (
        "a new query searches again",
        BOTH,
        finding(found(3, 1), 0),
        TextIn::Find(DOG),
        TextStage::Finding {
            query: DOG,
            hits: FindHits::Pending,
            place: place(0, On, Rendered),
        },
        &[TextOut::Find(FindOut::Search(DOG))],
    ),
    (
        "switching view while finding searches the new view",
        BOTH,
        finding(found(3, 1), 0),
        TextIn::ToggleSource,
        TextStage::Finding {
            query: CAT,
            hits: FindHits::Pending,
            place: place(0, On, Source),
        },
        &[TextOut::Show(Source), TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "scrolling while finding keeps the find",
        BOTH,
        finding(found(3, 1), 0),
        TextIn::Scroll(LineIndex(9)),
        TextStage::Finding {
            query: CAT,
            hits: found(3, 1),
            place: place(9, On, Rendered),
        },
        &[TextOut::Remember(Resume::Text { line: LineIndex(9) })],
    ),
    (
        "a restore while finding is ignored",
        BOTH,
        finding(found(3, 1), 0),
        TextIn::Restore(Resume::Text { line: LineIndex(9) }),
        finding(found(3, 1), 0),
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, views, from, input, state, outs) in CASES {
        let params = TextParams { views: *views };
        let (next, out) = from.clone().step(input.clone(), Stamp(0), &params);
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn a_stage_opens_rendered_only_when_there_is_a_rendering() {
    assert_eq!(TextStage::opened(BOTH), reading(0, On, Rendered));
    assert_eq!(TextStage::opened(SOURCE_ONLY), reading(0, On, Source));
}
