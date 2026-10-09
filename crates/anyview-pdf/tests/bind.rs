//! Several PDFs bound into one with an outline: the pages in order, each bookmark on the page its
//! part begins at.

#![allow(clippy::unwrap_used)]

use anyview_core::{PageIndex, PixelLen, PixelSize};
use anyview_pdf::{Bookmark, PagePicture, PdfDocument, bind, outline, pdf_of_pictures};

/// A PDF of pages of the given widths (each 10 tall), flat grey.
fn part(widths: &[u32]) -> Vec<u8> {
    let pictures: Vec<PagePicture> = widths
        .iter()
        .map(|width| PagePicture::Pixels {
            size: PixelSize {
                width: PixelLen(*width),
                height: PixelLen(10),
            },
            rgba: [128u8, 128, 128, 255]
                .iter()
                .copied()
                .cycle()
                .take((*width as usize) * 10 * 4)
                .collect(),
        })
        .collect();
    pdf_of_pictures(&pictures).unwrap()
}

fn mark(title: &str, depth: usize, part: usize) -> Bookmark {
    Bookmark {
        title: title.to_owned(),
        depth,
        part,
        find: None,
    }
}

#[test]
fn the_parts_become_one_document_in_order() {
    let bytes = bind(&[part(&[40, 50]), part(&[60]), part(&[70, 80])], &[]).unwrap();
    let document = PdfDocument::from_bytes(bytes).unwrap();
    let widths: Vec<u32> = document
        .page_sizes()
        .iter()
        .map(|size| size.width.0 / 750)
        .collect();
    assert_eq!(widths, [40, 50, 60, 70, 80]);
}

#[test]
fn each_bookmark_leads_to_the_page_its_part_begins_at() {
    let marks = [
        mark("One", 0, 0),
        mark("One, a", 1, 0),
        mark("Two", 0, 1),
        mark("Three", 0, 2),
    ];
    let bytes = bind(&[part(&[40, 50]), part(&[60]), part(&[70, 80])], &marks).unwrap();
    let document = PdfDocument::from_bytes(bytes).unwrap();
    let outline = outline(&document);
    let got: Vec<(&str, u32, Option<PageIndex>)> = outline
        .iter()
        .map(|entry| (entry.title.as_str(), entry.depth, entry.page))
        .collect();
    assert_eq!(
        got,
        [
            ("One", 0, Some(PageIndex(0))),
            ("One, a", 1, Some(PageIndex(0))),
            ("Two", 0, Some(PageIndex(2))),
            ("Three", 0, Some(PageIndex(3))),
        ]
    );
}

#[test]
fn a_bookmark_of_a_part_that_is_not_there_is_left_out() {
    let bytes = bind(&[part(&[40])], &[mark("One", 0, 0), mark("Gone", 0, 4)]).unwrap();
    let titles: Vec<String> = outline(&PdfDocument::from_bytes(bytes).unwrap())
        .into_iter()
        .map(|entry| entry.title)
        .collect();
    assert_eq!(titles, ["One"]);
}

#[test]
fn a_part_that_is_not_a_pdf_is_refused() {
    assert!(bind(&[b"not a pdf".to_vec()], &[]).is_err());
}
