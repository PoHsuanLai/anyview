//! Search across the document: hits by page and rectangle, the options, and where the rectangles
//! land on a drawn page, turned or not.

use crate::support;

use anyview_core::{Dpi, Edit, PageIndex, QuarterTurn};
use anyview_pdf::{
    CaseMatch, PageRect, PdfDocument, PdfWorker, SearchQuery, Stop, WordMatch, apply, page_op,
    search_document, search_page,
};
use support::{drawn, fixture, has_ink};

fn pages_of(doc: &PdfDocument, query: &SearchQuery) -> Vec<u32> {
    let (hits, _) = search_document(doc, &mut PdfWorker::new(), query, &Stop::new());
    hits.iter().map(|hit| hit.page.0).collect()
}

#[test]
fn hits_come_in_document_order_under_each_option() {
    let doc = fixture();
    let query = |text: &str, case: CaseMatch, words: WordMatch| SearchQuery {
        text: text.into(),
        case,
        words,
    };
    // name, query, pages of the hits
    let cases: [(&str, SearchQuery, &[u32]); 8] = [
        (
            "part of a word, any case",
            query("fox", CaseMatch::Ignore, WordMatch::Part),
            &[0, 1, 1],
        ),
        (
            "whole word only",
            query("fox", CaseMatch::Ignore, WordMatch::Whole),
            &[0, 1],
        ),
        (
            "exact case",
            query("Fox", CaseMatch::Exact, WordMatch::Part),
            &[1],
        ),
        (
            "exact case, whole word",
            query("Fox", CaseMatch::Exact, WordMatch::Whole),
            &[],
        ),
        (
            "a phrase across words",
            query("quick brown", CaseMatch::Ignore, WordMatch::Part),
            &[0],
        ),
        (
            "a word on the last page",
            query("appendix", CaseMatch::Ignore, WordMatch::Part),
            &[2],
        ),
        (
            "nothing",
            query("zebra", CaseMatch::Ignore, WordMatch::Part),
            &[],
        ),
        (
            "an empty query",
            query("  ", CaseMatch::Ignore, WordMatch::Part),
            &[],
        ),
    ];
    for (name, query, want) in cases {
        assert_eq!(pages_of(&doc, &query), want, "{name}");
    }
}

#[test]
fn a_hit_is_found_by_its_index_and_the_nearest_to_a_page_is_known() {
    let doc = fixture();
    let (hits, _) = search_document(
        &doc,
        &mut PdfWorker::new(),
        &SearchQuery::new("fox"),
        &Stop::new(),
    );
    assert_eq!(hits.len(), 3);
    assert_eq!(hits.get(0).map(|h| h.page), Some(PageIndex(0)));
    assert_eq!(hits.get(2).map(|h| h.page), Some(PageIndex(1)));
    assert!(hits.get(3).is_none());
    // name, reader's page, index of the nearest hit
    for (name, page, want) in [
        ("first page", 0, 0),
        ("second page", 1, 1),
        ("past the last hit", 2, 2),
    ] {
        assert_eq!(hits.nearest(PageIndex(page)), want, "{name}");
    }
    assert_eq!(anyview_pdf::Hits::default().nearest(PageIndex(4)), 0);
}

#[test]
fn a_hit_marks_the_words_on_the_drawn_page() {
    let doc = fixture();
    let mut worker = PdfWorker::new();
    let hits = search_page(&doc, &mut worker, PageIndex(0), &SearchQuery::new("fox")).expect("ok");
    assert_eq!(hits.len(), 1);
    let page = drawn(&doc, 0, Dpi::PAGE);
    let (width, height) = (page.size().width.0, page.size().height.0);
    let [rect] = hits[0].rects[..] else {
        panic!("one line: {:?}", hits[0].rects)
    };
    let (x, y, w, h) = pixels(rect, width, height);
    assert!(has_ink(&page, x, y, w, h), "ink under the hit");
    // The same box moved to the empty right-hand side of the line is blank.
    assert!(!has_ink(&page, 500, y, 50, h));
    // "fox" is the fourth word of a line that starts at x = 72 and is 14 points high.
    assert!((70..220).contains(&x) && h < 30, "{x} {w} {h}");
}

#[test]
fn a_hit_still_marks_the_words_after_the_page_is_turned() {
    let doc = fixture();
    for turn in [
        QuarterTurn::Quarter,
        QuarterTurn::Half,
        QuarterTurn::ThreeQuarter,
    ] {
        let op = page_op(Edit::Rotate(turn), PageIndex(0)).expect("ok");
        let turned = PdfDocument::from_bytes(apply(&doc, &[op]).expect("ok")).expect("ok");
        let hits = search_page(
            &turned,
            &mut PdfWorker::new(),
            PageIndex(0),
            &SearchQuery::new("fox"),
        )
        .expect("ok");
        let page = drawn(&turned, 0, Dpi::PAGE);
        let (width, height) = (page.size().width.0, page.size().height.0);
        let [rect] = hits[0].rects[..] else {
            panic!("one line")
        };
        let (x, y, w, h) = pixels(rect, width, height);
        assert!(
            has_ink(&page, x, y, w, h),
            "{turn:?}: ink under the hit at {x},{y} {w}x{h}"
        );
    }
}

/// The rectangle's pixel box on a page `width` x `height` pixels, kept inside it.
fn pixels(rect: PageRect, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let at = |fraction: u32, whole: u32| fraction * whole / 1000;
    let (left, right) = (
        at(rect.left.0, width),
        at(rect.right.0, width).min(width - 1),
    );
    let (top, bottom) = (
        at(rect.top.0, height),
        at(rect.bottom.0, height).min(height - 1),
    );
    (left, top, (right - left).max(1), (bottom - top).max(1))
}
