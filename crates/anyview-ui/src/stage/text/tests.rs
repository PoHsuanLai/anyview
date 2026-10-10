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
const fn edited(changes: Changes) -> Edited {
    Edited {
        changes,
        outside: Outside::Unchanged,
    }
}
/// Editing the source from `line`, with no find up.
const fn editing(line: u32, changes: Changes) -> TextStage {
    TextStage::Editing {
        place: place(line, On, Source),
        edited: edited(changes),
        find: None,
    }
}
/// Editing the source from `line` with a find for `cat` up, where its search stands at `hits`.
const fn editing_found(line: u32, hits: FindHits) -> TextStage {
    TextStage::Editing {
        place: place(line, On, Source),
        edited: edited(Changes::Saved),
        find: Some(EditFind { query: CAT, hits }),
    }
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
        "toggling wrap flips it, and the choice is kept for the kind",
        BOTH,
        reading(5, On, Rendered),
        TextIn::ToggleWrap,
        reading(5, Off, Rendered),
        &[TextOut::Wrapped(Off)],
    ),
    (
        "toggling wrap flips it back",
        BOTH,
        reading(5, Off, Rendered),
        TextIn::ToggleWrap,
        reading(5, On, Rendered),
        &[TextOut::Wrapped(On)],
    ),
    (
        "editing starts in the source and asks for the file's text",
        SOURCE_ONLY,
        reading(5, On, Source),
        TextIn::Edit,
        editing(5, Changes::Saved),
        &[TextOut::BeginEdit],
    ),
    (
        "editing a Markdown page switches to its source first",
        BOTH,
        reading(5, On, Rendered),
        TextIn::Edit,
        editing(5, Changes::Saved),
        &[TextOut::Show(Source), TextOut::BeginEdit],
    ),
    (
        "editing from a find carries the find over",
        BOTH,
        finding(found(3, 1), 4),
        TextIn::Edit,
        editing_found(4, found(3, 1)),
        &[TextOut::Show(Source), TextOut::BeginEdit],
    ),
    (
        "Done goes back to reading and lets the text go",
        SOURCE_ONLY,
        editing(5, Changes::Saved),
        TextIn::Done,
        reading(5, On, Source),
        &[TextOut::EndEdit],
    ),
    (
        "Done with a find up clears its marks too",
        SOURCE_ONLY,
        editing_found(5, found(3, 1)),
        TextIn::Done,
        reading(5, On, Source),
        &[TextOut::EndEdit, TextOut::Find(FindOut::Clear)],
    ),
    (
        "a change marks the text unsaved",
        SOURCE_ONLY,
        editing(5, Changes::Saved),
        TextIn::Edited(Changes::Unsaved),
        editing(5, Changes::Unsaved),
        &[],
    ),
    (
        "a change searches a find over the text again",
        SOURCE_ONLY,
        editing_found(5, found(3, 1)),
        TextIn::Edited(Changes::Unsaved),
        TextStage::Editing {
            place: place(5, On, Source),
            edited: edited(Changes::Unsaved),
            find: Some(EditFind {
                query: CAT,
                hits: found(3, 1),
            }),
        },
        &[TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "the hits of a find just typed show the nearest",
        SOURCE_ONLY,
        editing_found(5, FindHits::Pending),
        TextIn::Results {
            query: CAT,
            count: HitCount(3),
            nearest: HitIndex(1),
        },
        editing_found(5, found(3, 1)),
        &[show_hit(1)],
    ),
    (
        "the hits of a search made after a change do not move the view",
        SOURCE_ONLY,
        editing_found(5, found(3, 1)),
        TextIn::Results {
            query: CAT,
            count: HitCount(2),
            nearest: HitIndex(0),
        },
        editing_found(5, found(2, 0)),
        &[],
    ),
    (
        "a find typed while editing searches the text",
        SOURCE_ONLY,
        editing(5, Changes::Saved),
        TextIn::Find(CAT),
        editing_found(5, FindHits::Pending),
        &[TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "stepping through the hits of a find while editing shows each",
        SOURCE_ONLY,
        editing_found(5, found(3, 2)),
        TextIn::NextHit,
        editing_found(5, found(3, 0)),
        &[show_hit(0)],
    ),
    (
        "closing the find keeps the editing",
        SOURCE_ONLY,
        editing_found(5, found(3, 2)),
        TextIn::CloseFind,
        editing(5, Changes::Saved),
        &[TextOut::Find(FindOut::Clear)],
    ),
    (
        "Save asks for the text to be written",
        SOURCE_ONLY,
        editing(5, Changes::Unsaved),
        TextIn::Save,
        editing(5, Changes::Unsaved),
        &[TextOut::Save],
    ),
    (
        "the file changing on disk is remembered",
        SOURCE_ONLY,
        editing(5, Changes::Unsaved),
        TextIn::Disk(Outside::Changed),
        TextStage::Editing {
            place: place(5, On, Source),
            edited: Edited {
                changes: Changes::Unsaved,
                outside: Outside::Changed,
            },
            find: None,
        },
        &[],
    ),
    (
        "the view of an edited text is its source and stays so",
        BOTH,
        editing(5, Changes::Saved),
        TextIn::ToggleSource,
        editing(5, Changes::Saved),
        &[],
    ),
    (
        "toggling wrap while editing keeps the choice",
        SOURCE_ONLY,
        editing(5, Changes::Saved),
        TextIn::ToggleWrap,
        TextStage::Editing {
            place: place(5, Off, Source),
            edited: edited(Changes::Saved),
            find: None,
        },
        &[TextOut::Wrapped(Off)],
    ),
    (
        "scrolling while editing is remembered",
        SOURCE_ONLY,
        editing(5, Changes::Unsaved),
        TextIn::Scroll(LineIndex(40)),
        editing(40, Changes::Unsaved),
        &[remembered(40)],
    ),
    (
        "editing again while editing is nothing",
        SOURCE_ONLY,
        editing(5, Changes::Unsaved),
        TextIn::Edit,
        editing(5, Changes::Unsaved),
        &[],
    ),
    (
        "Done, Save and a change mean nothing to a text being read",
        SOURCE_ONLY,
        reading(5, On, Source),
        TextIn::Done,
        reading(5, On, Source),
        &[],
    ),
    (
        "find starts a search",
        BOTH,
        reading(5, On, Source),
        TextIn::Find(CAT),
        TextStage::Finding {
            query: CAT,
            hits: FindHits::Pending,
            place: place(5, On, Source),
        },
        &[TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "a find in a rendered page starts in the source, where hits can be marked",
        BOTH,
        reading(5, On, Rendered),
        TextIn::Find(CAT),
        TextStage::Finding {
            query: CAT,
            hits: FindHits::Pending,
            place: place(5, On, Source),
        },
        &[TextOut::Show(Source), TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "a find in a file with only source stays in it",
        SOURCE_ONLY,
        reading(5, On, Rendered),
        TextIn::Find(CAT),
        TextStage::Finding {
            query: CAT,
            hits: FindHits::Pending,
            place: place(5, On, Rendered),
        },
        &[TextOut::Find(FindOut::Search(CAT))],
    ),
    (
        "an empty find opens the find bar with nothing to search",
        BOTH,
        reading(5, On, Source),
        TextIn::Find(TypedText::EMPTY),
        TextStage::Finding {
            query: TypedText::EMPTY,
            hits: FindHits::Idle,
            place: place(5, On, Source),
        },
        &[TextOut::Find(FindOut::Clear)],
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
        "clearing the query keeps the bar open and clears the marks",
        BOTH,
        finding(found(3, 1), 7),
        TextIn::Find(TypedText::EMPTY),
        TextStage::Finding {
            query: TypedText::EMPTY,
            hits: FindHits::Idle,
            place: place(7, On, Rendered),
        },
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
        let params = TextParams {
            views: *views,
            editable: Editable::Yes,
            ..TextParams::default()
        };
        let (next, out) = from.clone().step(input.clone(), Stamp(0), &params, &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn a_file_that_is_not_editable_is_not_edited() {
    let params = TextParams::default();
    assert_eq!(params.editable, Editable::No);
    let before = finding(found(3, 1), 4);
    let (next, outs) = before.clone().step(TextIn::Edit, Stamp(0), &params, &());
    assert_eq!(next, before, "a find stays a find");
    assert_eq!(outs, vec![]);
    let (next, outs) = reading(5, On, Source).step(TextIn::Edit, Stamp(0), &params, &());
    assert_eq!(next, reading(5, On, Source));
    assert_eq!(outs, vec![]);
}

#[test]
fn a_stage_opens_rendered_only_when_there_is_a_rendering() {
    assert_eq!(TextStage::opened(BOTH), reading(0, On, Rendered));
    assert_eq!(TextStage::opened(SOURCE_ONLY), reading(0, On, Source));
}

const EXTENT: TextExtent = TextExtent {
    lines: LineTotal(100),
    page: PageLines(20),
};

/// Name, state before, step, state after, outputs.
type StepCase = (
    &'static str,
    TextStage,
    TextStep,
    TextStage,
    &'static [TextOut],
);

const fn remembered(line: u32) -> TextOut {
    TextOut::Remember(Resume::Text {
        line: LineIndex(line),
    })
}

const STEP_CASES: &[StepCase] = &[
    (
        "page down moves a page and is remembered",
        reading(10, On, Source),
        TextStep::PageDown,
        reading(30, On, Source),
        &[remembered(30)],
    ),
    (
        "end goes to the last page",
        reading(10, On, Source),
        TextStep::Bottom,
        reading(80, On, Source),
        &[remembered(80)],
    ),
    (
        "a step that goes nowhere is not remembered",
        reading(0, On, Source),
        TextStep::LineUp,
        reading(0, On, Source),
        &[],
    ),
    (
        "stepping keeps the find",
        finding(found(3, 1), 10),
        TextStep::LineDown,
        TextStage::Finding {
            query: CAT,
            hits: found(3, 1),
            place: place(11, On, Rendered),
        },
        &[remembered(11)],
    ),
];

#[test]
fn every_key_step_of_the_table_moves_the_line_as_written() {
    let params = TextParams {
        extent: EXTENT,
        ..TextParams::default()
    };
    for (name, from, step, state, outs) in STEP_CASES {
        let (next, out) = from
            .clone()
            .step(TextIn::Step(*step), Stamp(0), &params, &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
    }
}
