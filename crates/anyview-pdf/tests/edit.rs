//! Page edits as data, written to bytes and read back.

mod support;

use anyview_core::{Edit, PageIndex, PageRange, QuarterTurn};
use anyview_pdf::{
    PageOp, PageSize, PdfDocument, PdfError, PdfWorker, SearchQuery, Stop, apply, outline, page_op,
    search_document,
};
use support::fixture;

fn reopened(doc: &PdfDocument, ops: &[PageOp]) -> PdfDocument {
    PdfDocument::from_bytes(apply(doc, ops).expect("applies")).expect("opens")
}

fn first_hit_page(doc: &PdfDocument, text: &str) -> Option<u32> {
    let (hits, _) = search_document(
        doc,
        &mut PdfWorker::new(),
        &SearchQuery::new(text),
        &Stop::new(),
    );
    hits.get(0).map(|hit| hit.page.0)
}

fn size(doc: &PdfDocument, page: u32) -> (u32, u32) {
    let PageSize { width, height } = doc.page_size(PageIndex(page)).expect("a page");
    (width.0 / 1000, height.0 / 1000)
}

#[test]
fn a_rotation_turns_one_page_and_turns_add_up() {
    let doc = fixture();
    let quarter = PageOp::Rotate {
        page: PageIndex(0),
        turn: QuarterTurn::Quarter,
    };
    let once = reopened(&doc, &[quarter]);
    assert_eq!((size(&once, 0), size(&once, 1)), ((792, 612), (612, 792)));
    let twice = reopened(&doc, &[quarter, quarter]);
    assert_eq!(size(&twice, 0), (612, 792));
    let back = reopened(
        &once,
        &[PageOp::Rotate {
            page: PageIndex(0),
            turn: QuarterTurn::ThreeQuarter,
        }],
    );
    assert_eq!(size(&back, 0), (612, 792));
    // The original is untouched.
    assert_eq!(size(&doc, 0), (612, 792));
}

#[test]
fn deleting_pages_removes_exactly_those_and_keeps_the_rest() {
    let doc = fixture();
    let first = PageRange::single(PageIndex(0));
    let left = reopened(&doc, &[PageOp::Delete(first)]);
    assert_eq!(left.page_count().get(), 2);
    assert_eq!(first_hit_page(&left, "Chapter Two"), Some(0));
    assert_eq!(first_hit_page(&left, "Appendix"), Some(1));
    assert_eq!(first_hit_page(&left, "Chapter One"), None);
    // The run is cut to the document; a range starting past the end is refused.
    let tail = PageRange::new(PageIndex(1), PageIndex(9)).unwrap();
    assert_eq!(
        reopened(&doc, &[PageOp::Delete(tail)]).page_count().get(),
        1
    );
    let beyond = PageRange::single(PageIndex(7));
    assert!(matches!(
        apply(&doc, &[PageOp::Delete(beyond)]),
        Err(PdfError::PageOutOfRange { .. })
    ));
}

#[test]
fn deleting_every_page_is_refused() {
    let doc = fixture();
    let all = PageRange::new(PageIndex(0), PageIndex(2)).unwrap();
    assert!(matches!(
        apply(&doc, &[PageOp::Delete(all)]),
        Err(PdfError::WouldDeleteAll)
    ));
}

#[test]
fn moving_a_page_reorders_the_rest() {
    let doc = fixture();
    let moved = reopened(
        &doc,
        &[PageOp::Move {
            from: PageIndex(0),
            to: PageIndex(2),
        }],
    );
    assert_eq!(moved.page_count().get(), 3);
    assert_eq!(first_hit_page(&moved, "Chapter Two"), Some(0));
    assert_eq!(first_hit_page(&moved, "Appendix"), Some(1));
    assert_eq!(first_hit_page(&moved, "Chapter One"), Some(2));
    assert_eq!((size(&moved, 1), size(&moved, 2)), ((400, 300), (612, 792)));
    assert!(matches!(
        apply(
            &doc,
            &[PageOp::Move {
                from: PageIndex(0),
                to: PageIndex(3)
            }]
        ),
        Err(PdfError::PageOutOfRange { .. })
    ));
}

#[test]
fn edits_apply_in_order_each_seeing_the_pages_before_it_left() {
    let doc = fixture();
    let ops = [
        PageOp::Move {
            from: PageIndex(2),
            to: PageIndex(0),
        },
        PageOp::Rotate {
            page: PageIndex(0),
            turn: QuarterTurn::Quarter,
        },
        PageOp::Delete(PageRange::single(PageIndex(1))),
    ];
    let result = reopened(&doc, &ops);
    assert_eq!(result.page_count().get(), 2);
    // The 400 x 300 appendix came first and was turned; the old first page was deleted.
    assert_eq!(size(&result, 0), (300, 400));
    assert_eq!(first_hit_page(&result, "Chapter Two"), Some(1));
    assert_eq!(first_hit_page(&result, "Chapter One"), None);
    assert!(!outline(&result).is_empty() || outline(&doc).len() == 4);
    // No edits is the file as it was.
    assert_eq!(apply(&doc, &[]).unwrap(), doc.bytes());
}

#[test]
fn a_viewer_edit_reaches_the_file() {
    let doc = fixture();
    let op = page_op(Edit::Rotate(QuarterTurn::Half), PageIndex(1)).unwrap();
    let result = reopened(&doc, &[op]);
    assert_eq!(size(&result, 1), (612, 792));
    assert!(page_op(Edit::Flip(anyview_core::Axis::Horizontal), PageIndex(0)).is_err());
}

#[test]
fn what_pointed_at_a_deleted_page_points_nowhere() {
    let doc = fixture();
    let left = reopened(&doc, &[PageOp::Delete(PageRange::single(PageIndex(0)))]);
    let pages: Vec<(String, Option<u32>)> = outline(&left)
        .into_iter()
        .map(|entry| (entry.title, entry.page.map(|page| page.0)))
        .collect();
    assert!(pages.contains(&("Chapter One".to_owned(), None)));
    assert!(pages.contains(&("Chapter Two".to_owned(), Some(0))));
    assert!(pages.contains(&("Appendix".to_owned(), Some(1))));
    assert!(
        !pages.iter().any(|(_, page)| *page == Some(2)),
        "no entry leads past the end"
    );
}

#[test]
fn a_signed_file_is_known_by_its_byte_range() {
    let plain = fixture();
    assert!(!plain.is_signed());
    let mut bytes = plain.bytes().to_vec();
    bytes.extend_from_slice(b"\n% /Type /Sig /ByteRange [0 10 20 30]\n");
    let signed = PdfDocument::from_bytes(bytes).expect("still opens");
    assert!(signed.is_signed());
}
