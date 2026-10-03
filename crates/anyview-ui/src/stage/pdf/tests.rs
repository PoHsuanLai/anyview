use super::super::find::{FindHits, FindOut, HitCount, HitIndex};
use super::super::zoom::{Viewport, ZoomDir};
use super::*;
use crate::typed::TypedText;
use anyview_core::{PageCount, PageIndex, Permille, Resume, Zoom};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

/// A ten-page document in a window where fit is 80% and the view shows 100%.
fn params() -> PdfParams {
    PdfParams {
        pages: PageCount::new(10).unwrap(),
        viewport: Viewport {
            shown: Permille(1000),
            fit: Permille(800),
        },
        step: Permille(1250),
    }
}
const CAT: TypedText = TypedText::from_static("cat");
const DOG: TypedText = TypedText::from_static("dog");

const fn at(page: u32, offset: u32, zoom: Zoom) -> PageView {
    PageView {
        page: PageIndex(page),
        offset: Permille(offset),
        zoom,
    }
}
const fn to(page: u32, offset: u32) -> Destination {
    Destination {
        page: PageIndex(page),
        offset: Permille(offset),
    }
}
const fn reading(page: u32, offset: u32) -> PdfStage {
    PdfStage::Reading {
        view: at(page, offset, Zoom::Fit),
    }
}
const fn finding(hits: FindHits, page: u32) -> PdfStage {
    PdfStage::Finding {
        query: CAT,
        hits,
        view: at(page, 0, Zoom::Fit),
    }
}
const fn jumping(page: u32, offset: u32) -> PdfStage {
    PdfStage::Jumping {
        target: to(page, offset),
        view: at(0, 0, Zoom::Fit),
    }
}
const fn remember(page: u32, offset: u32, zoom: Zoom) -> PdfOut {
    PdfOut::Remember(Resume::Pdf {
        page: PageIndex(page),
        offset: Permille(offset),
        zoom,
    })
}
const fn found(count: u32, current: u32) -> FindHits {
    FindHits::answered(HitCount(count), HitIndex(current))
}
const fn show(hit: u32) -> PdfOut {
    PdfOut::Find(FindOut::ShowHit(HitIndex(hit)))
}

/// Name, state before, input, state after, outputs.
type Case = (&'static str, PdfStage, PdfIn, PdfStage, &'static [PdfOut]);

const CASES: &[Case] = &[
    (
        "scrolling moves the view and is remembered",
        reading(0, 0),
        PdfIn::Scroll {
            page: PageIndex(3),
            offset: Permille(250),
        },
        reading(3, 250),
        &[remember(3, 250, Zoom::Fit)],
    ),
    (
        "scrolling past the end stays inside the document",
        reading(0, 0),
        PdfIn::Scroll {
            page: PageIndex(99),
            offset: Permille(2000),
        },
        reading(9, 1000),
        &[remember(9, 1000, Zoom::Fit)],
    ),
    (
        "a zoom past the limit is clamped",
        reading(2, 0),
        PdfIn::SetZoom(Zoom::Scale(Permille(999_999))),
        PdfStage::Reading {
            view: at(2, 0, Zoom::Scale(Zoom::MAX_SCALE)),
        },
        &[remember(2, 0, Zoom::Scale(Zoom::MAX_SCALE))],
    ),
    (
        "a zoom step in scales what is shown",
        reading(2, 0),
        PdfIn::ZoomStep(ZoomDir::In),
        PdfStage::Reading {
            view: at(2, 0, Zoom::Scale(Permille(1250))),
        },
        &[remember(2, 0, Zoom::Scale(Permille(1250)))],
    ),
    (
        "find starts a search",
        reading(2, 0),
        PdfIn::Find(CAT),
        finding(FindHits::Pending, 2),
        &[PdfOut::Find(FindOut::Search(CAT))],
    ),
    (
        "an empty find opens the find bar with nothing to search",
        reading(2, 0),
        PdfIn::Find(TypedText::EMPTY),
        PdfStage::Finding {
            query: TypedText::EMPTY,
            hits: FindHits::Idle,
            view: at(2, 0, Zoom::Fit),
        },
        &[PdfOut::Find(FindOut::Clear)],
    ),
    (
        "go to jumps and asks for the scroll",
        reading(0, 0),
        PdfIn::GoTo(to(5, 300)),
        jumping(5, 300),
        &[PdfOut::ScrollTo(to(5, 300))],
    ),
    (
        "go to past the end lands on the last page",
        reading(0, 0),
        PdfIn::GoTo(to(99, 0)),
        jumping(9, 0),
        &[PdfOut::ScrollTo(to(9, 0))],
    ),
    (
        "next page jumps to the top of the following page",
        reading(0, 400),
        PdfIn::NextPage,
        PdfStage::Jumping {
            target: to(1, 0),
            view: at(0, 400, Zoom::Fit),
        },
        &[PdfOut::ScrollTo(to(1, 0))],
    ),
    (
        "next page on the last page is nothing",
        reading(9, 0),
        PdfIn::NextPage,
        reading(9, 0),
        &[],
    ),
    (
        "previous page on the first page is nothing",
        reading(0, 0),
        PdfIn::PreviousPage,
        reading(0, 0),
        &[],
    ),
    (
        "restoring a stored place scrolls to it",
        reading(0, 0),
        PdfIn::Restore(Resume::Pdf {
            page: PageIndex(4),
            offset: Permille(500),
            zoom: Zoom::Actual,
        }),
        PdfStage::Reading {
            view: at(4, 500, Zoom::Actual),
        },
        &[PdfOut::ScrollTo(to(4, 500))],
    ),
    (
        "restoring another format's place is ignored",
        reading(0, 0),
        PdfIn::Restore(Resume::Nothing),
        reading(0, 0),
        &[],
    ),
    (
        "the hits arrive and the nearest is shown",
        finding(FindHits::Pending, 0),
        PdfIn::Results {
            query: CAT,
            count: HitCount(5),
            nearest: HitIndex(2),
        },
        finding(found(5, 2), 0),
        &[show(2)],
    ),
    (
        "hits for an earlier query are ignored",
        finding(FindHits::Pending, 0),
        PdfIn::Results {
            query: DOG,
            count: HitCount(5),
            nearest: HitIndex(2),
        },
        finding(FindHits::Pending, 0),
        &[],
    ),
    (
        "a search with no hits says so",
        finding(FindHits::Pending, 0),
        PdfIn::Results {
            query: CAT,
            count: HitCount(0),
            nearest: HitIndex(0),
        },
        finding(FindHits::NoMatch, 0),
        &[],
    ),
    (
        "next hit steps forward",
        finding(found(5, 1), 0),
        PdfIn::NextHit,
        finding(found(5, 2), 0),
        &[show(2)],
    ),
    (
        "next hit after the last wraps to the first",
        finding(found(5, 4), 0),
        PdfIn::NextHit,
        finding(found(5, 0), 0),
        &[show(0)],
    ),
    (
        "previous hit before the first wraps to the last",
        finding(found(5, 0), 0),
        PdfIn::PreviousHit,
        finding(found(5, 4), 0),
        &[show(4)],
    ),
    (
        "a single hit wraps onto itself",
        finding(found(1, 0), 0),
        PdfIn::NextHit,
        finding(found(1, 0), 0),
        &[show(0)],
    ),
    (
        "next hit while the search is pending is nothing",
        finding(FindHits::Pending, 0),
        PdfIn::NextHit,
        finding(FindHits::Pending, 0),
        &[],
    ),
    (
        "next hit with no matches is nothing",
        finding(FindHits::NoMatch, 0),
        PdfIn::NextHit,
        finding(FindHits::NoMatch, 0),
        &[],
    ),
    (
        "a new query searches again",
        finding(found(5, 1), 0),
        PdfIn::Find(DOG),
        PdfStage::Finding {
            query: DOG,
            hits: FindHits::Pending,
            view: at(0, 0, Zoom::Fit),
        },
        &[PdfOut::Find(FindOut::Search(DOG))],
    ),
    (
        "closing the find returns to reading where the reader is",
        finding(found(5, 1), 3),
        PdfIn::CloseFind,
        reading(3, 0),
        &[PdfOut::Find(FindOut::Clear)],
    ),
    (
        "clearing the query keeps the bar open and clears the marks",
        finding(found(5, 1), 3),
        PdfIn::Find(TypedText::EMPTY),
        PdfStage::Finding {
            query: TypedText::EMPTY,
            hits: FindHits::Idle,
            view: at(3, 0, Zoom::Fit),
        },
        &[PdfOut::Find(FindOut::Clear)],
    ),
    (
        "scrolling while finding keeps the find",
        finding(found(5, 1), 0),
        PdfIn::Scroll {
            page: PageIndex(2),
            offset: Permille(0),
        },
        PdfStage::Finding {
            query: CAT,
            hits: found(5, 1),
            view: at(2, 0, Zoom::Fit),
        },
        &[remember(2, 0, Zoom::Fit)],
    ),
    (
        "a page key while finding scrolls and keeps the find",
        finding(found(5, 1), 0),
        PdfIn::NextPage,
        finding(found(5, 1), 0),
        &[PdfOut::ScrollTo(to(1, 0))],
    ),
    (
        "arriving ends the jump on the target",
        jumping(5, 300),
        PdfIn::Arrived,
        reading(5, 300),
        &[remember(5, 300, Zoom::Fit)],
    ),
    (
        "the reader scrolling ends the jump",
        jumping(5, 300),
        PdfIn::Scroll {
            page: PageIndex(2),
            offset: Permille(100),
        },
        reading(2, 100),
        &[remember(2, 100, Zoom::Fit)],
    ),
    (
        "another destination retargets the jump",
        jumping(5, 300),
        PdfIn::GoTo(to(7, 0)),
        jumping(7, 0),
        &[PdfOut::ScrollTo(to(7, 0))],
    ),
    (
        "a page key during a jump goes on from its target",
        jumping(5, 300),
        PdfIn::NextPage,
        jumping(6, 0),
        &[PdfOut::ScrollTo(to(6, 0))],
    ),
    (
        "a find during a jump starts from the target",
        jumping(5, 300),
        PdfIn::Find(CAT),
        PdfStage::Finding {
            query: CAT,
            hits: FindHits::Pending,
            view: at(5, 300, Zoom::Fit),
        },
        &[PdfOut::Find(FindOut::Search(CAT))],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, from, input, state, outs) in CASES {
        let (next, out) = from.clone().step(input.clone(), Stamp(0), &params());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}
