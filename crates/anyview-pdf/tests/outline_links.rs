//! The outline and the links of a page.

mod support;

use anyview_core::{PageIndex, Permille};
use anyview_pdf::{Disclosure, LinkTarget, OutlineEntry, outline, page_links};
use support::fixture;

#[test]
fn the_outline_is_a_list_in_reading_order_with_depths_and_pages() {
    let entry = |title: &str, depth, page, disclosure| OutlineEntry {
        title: title.into(),
        depth,
        page: Some(PageIndex(page)),
        disclosure,
    };
    assert_eq!(
        outline(&fixture()),
        [
            entry("Chapter One", 0, 0, Disclosure::Open),
            entry("Section 1.1", 1, 0, Disclosure::Leaf),
            entry("Chapter Two", 0, 1, Disclosure::Leaf),
            entry("Appendix", 0, 2, Disclosure::Leaf),
        ]
    );
}

#[test]
fn a_page_without_links_has_none_and_a_missing_page_is_an_error() {
    let doc = fixture();
    assert!(page_links(&doc, PageIndex(1)).unwrap().is_empty());
    assert!(page_links(&doc, PageIndex(3)).is_err());
}

#[test]
fn links_say_where_they_are_and_where_they_lead() {
    let links = page_links(&fixture(), PageIndex(0)).unwrap();
    let targets: Vec<&LinkTarget> = links.iter().map(|link| &link.target).collect();
    assert_eq!(
        targets,
        [
            &LinkTarget::Page(PageIndex(2)),
            &LinkTarget::Uri("https://example.com/".into())
        ]
    );
    // [72 650 300 670] on a 612 x 792 page: x 72..300, and y 650..670 up from the bottom.
    let rect = links[0].rect;
    assert_eq!((rect.left, rect.right), (Permille(118), Permille(490)));
    assert_eq!((rect.top, rect.bottom), (Permille(154), Permille(179)));
}
