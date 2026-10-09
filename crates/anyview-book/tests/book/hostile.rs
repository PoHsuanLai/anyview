//! EPUBs built to hurt: nesting that overflows a parser's stack, a package of tens of thousands of
//! items, a stylesheet linked hundreds of times. Each is small, and each must be read in a bounded
//! time and memory.

use crate::support;

use anyview_book::{BookError, Epub};
use anyview_core::SectionIndex;
use std::time::{Duration, Instant};
use support::write_zip;

/// How long a bounded read may take: the quadratic ones took seconds to minutes.
const BOUND: Duration = Duration::from_secs(5);

const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

/// A package of `items` chapters in reading order, with the manifest's navigation document.
fn big_package(items: usize) -> String {
    let manifest: String = (0..items)
        .map(|n| {
            format!("<item id=\"c{n}\" href=\"c{n}.xhtml\" media-type=\"application/xhtml+xml\"/>")
        })
        .collect();
    let spine: String = (0..items)
        .map(|n| format!("<itemref idref=\"c{n}\"/>"))
        .collect();
    format!(
        "<package xmlns=\"http://www.idpf.org/2007/opf\"><metadata/><manifest>\
         <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\
         {manifest}</manifest><spine>{spine}</spine></package>"
    )
}

fn big_nav(items: usize) -> String {
    let entries: String = (0..items)
        .map(|n| format!("<li><a href=\"c{n}.xhtml\">Chapter {n}</a></li>"))
        .collect();
    format!("<html><body><nav><ol>{entries}</ol></nav></body></html>")
}

#[test]
fn a_package_of_tens_of_thousands_of_items_is_read_in_linear_time() {
    const ITEMS: usize = 60_000;
    let dir = tempfile::tempdir().unwrap();
    let package = big_package(ITEMS);
    let nav = big_nav(ITEMS);
    let path = write_zip(
        dir.path(),
        "quad.epub",
        &[
            ("META-INF/container.xml", CONTAINER.as_bytes()),
            ("OEBPS/content.opf", package.as_bytes()),
            ("OEBPS/nav.xhtml", nav.as_bytes()),
        ],
    );
    let started = Instant::now();
    let epub = Epub::open(&path).unwrap();
    assert_eq!(epub.sections().get() as usize, ITEMS);
    let contents = epub.contents();
    assert_eq!(contents.len(), ITEMS);
    assert_eq!(
        contents[ITEMS - 1].section,
        Some(SectionIndex(ITEMS as u32 - 1))
    );
    assert!(started.elapsed() < BOUND, "took {:?}", started.elapsed());
}

#[test]
fn parts_nested_past_the_parsers_stack_are_refused_on_a_small_stack() {
    let deep = format!("{}{}", "<a>".repeat(100_000), "</a>".repeat(100_000));
    let opf = big_package(2);
    let nav_with_depth = format!("<html><body><nav><ol>{deep}</ol></nav></body></html>");
    // name, container, package, navigation document
    let cases = [
        ("the container", deep.clone(), opf.clone(), big_nav(2)),
        (
            "the package",
            CONTAINER.to_owned(),
            format!("<package>{deep}</package>"),
            big_nav(2),
        ),
        (
            "the navigation document",
            CONTAINER.to_owned(),
            opf,
            nav_with_depth,
        ),
    ];
    for (name, container, package, nav) in cases {
        let dir = tempfile::tempdir().unwrap();
        let path = write_zip(
            dir.path(),
            "deep.epub",
            &[
                ("META-INF/container.xml", container.as_bytes()),
                ("OEBPS/content.opf", package.as_bytes()),
                ("OEBPS/nav.xhtml", nav.as_bytes()),
            ],
        );
        // The launcher's worker has 2 MiB of stack: 100 000 open elements overflow 8 MiB.
        let worker = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || match Epub::open(&path) {
                Ok(epub) => (Ok(epub.sections().get()), Ok(epub.contents().len())),
                Err(error) => (Err(error.clone()), Err(error)),
            })
            .unwrap();
        let (opened, titles) = worker
            .join()
            .expect("a deep part does not overflow the stack");
        match name {
            // Only the navigation document's own nesting is refused; the book still opens, and
            // its contents fall back to the chapters by number.
            "the navigation document" => {
                assert_eq!(opened, Ok(2), "{name}");
                assert_eq!(titles, Ok(2), "{name}");
            }
            _ => assert!(
                matches!(
                    opened,
                    Err(BookError::Xml { .. }) | Err(BookError::Missing(_))
                ),
                "{name}: {opened:?}"
            ),
        }
    }
}

/// One chapter that links `sheet` `links` times, in a package of one chapter.
fn linking_book(dir: &std::path::Path, sheets: &[(&str, Vec<u8>)], links: &[&str]) -> Epub {
    let head: String = links
        .iter()
        .map(|href| format!("<link rel=\"stylesheet\" href=\"{href}\"/>"))
        .collect();
    let chapter = format!("<html><head>{head}</head><body><p>x</p></body></html>");
    let package = big_package(1);
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/content.opf", package.as_bytes()),
        ("OEBPS/c0.xhtml", chapter.as_bytes()),
        ("OEBPS/nav.xhtml", b"<html/>"),
    ];
    entries.extend(sheets.iter().map(|(name, bytes)| (*name, bytes.as_slice())));
    Epub::open(&write_zip(dir, "css.epub", &entries)).unwrap()
}

#[test]
fn a_stylesheet_linked_hundreds_of_times_is_sealed_once() {
    let sheet = format!("{}\n", "p { color: red }".repeat(60_000)).into_bytes();
    let dir = tempfile::tempdir().unwrap();
    let links = vec!["s.css"; 400];
    let epub = linking_book(dir.path(), &[("OEBPS/s.css", sheet.clone())], &links);
    let started = Instant::now();
    let chapter = epub.chapter(SectionIndex(0)).unwrap();
    assert!(started.elapsed() < BOUND, "took {:?}", started.elapsed());
    // Once: the sheet and the newline that ends it, not four hundred times that.
    assert_eq!(chapter.styles.len(), sheet.len() + 1);
}

#[test]
fn the_sheets_of_a_chapter_count_against_what_it_may_take_in() {
    // Seven mebibytes each, under the 8 MiB a single asset may be, over the 24 MiB a chapter may
    // take: the sheets after the third are left out.
    let sheet = |n: usize| format!(".s{n}{{}}\n{}", "p{color:red}".repeat(600_000)).into_bytes();
    let names = [
        "OEBPS/a.css",
        "OEBPS/b.css",
        "OEBPS/c.css",
        "OEBPS/d.css",
        "OEBPS/e.css",
    ];
    let sheets: Vec<(&str, Vec<u8>)> = names
        .iter()
        .enumerate()
        .map(|(n, name)| (*name, sheet(n)))
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let epub = linking_book(
        dir.path(),
        &sheets,
        &["a.css", "b.css", "c.css", "d.css", "e.css"],
    );
    let chapter = epub.chapter(SectionIndex(0)).unwrap();
    let sealed = (0..5)
        .filter(|n| chapter.styles.contains(&format!(".s{n}{{}}")))
        .count();
    assert_eq!(
        sealed, 4,
        "the fourth starts under the cap, the fifth over it"
    );
    assert!(chapter.styles.len() < 5 * 7_300_000);
}
