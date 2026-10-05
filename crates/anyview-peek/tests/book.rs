//! Peeking at a book: its facts and its cover, from a tiny EPUB and a tiny comic built here.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FilePath;
use anyview_peek::{Body, peek, probe};
use std::io::Write;
use support::{Home, pane_budget, rows};

fn zip_at(dir: &std::path::Path, name: &str, entries: &[(&str, Vec<u8>)]) -> FilePath {
    let path = dir.join(name);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (entry, bytes) in entries {
        writer
            .start_file(*entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    FilePath::new(path).unwrap()
}

fn picture() -> Vec<u8> {
    std::fs::read(support::path(Home::Image, "quadrants.png")).unwrap()
}

const PACKAGE: &str = r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Peeked</dc:title>
<dc:creator>Ann</dc:creator><dc:language>fr</dc:language></metadata>
<manifest><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>
<item id="i" href="cover.png" media-type="image/png" properties="cover-image"/></manifest>
<spine><itemref idref="c"/></spine></package>"#;

const CONTAINER: &str = r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
<rootfile full-path="content.opf"/></rootfiles></container>"#;

#[test]
fn an_epub_shows_its_cover_over_its_facts() {
    let dir = tempfile::tempdir().unwrap();
    let file = zip_at(
        dir.path(),
        "book.epub",
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
            ("content.opf", PACKAGE.as_bytes().to_vec()),
            ("c.xhtml", b"<p>x</p>".to_vec()),
            ("cover.png", picture()),
        ],
    );
    let probed = probe(&file).unwrap();
    let peeked = peek(&probed.source, &probed.sniffed, &pane_budget());
    assert!(
        matches!(peeked.body, Body::Picture(_)),
        "{:?}",
        peeked.body.slug()
    );
    let facts = rows(&peeked.facts);
    for want in [
        ("title", "Peeked"),
        ("author", "Ann"),
        ("language", "fr"),
        ("chapters", "1"),
    ] {
        assert!(
            facts.iter().any(|(k, v)| (*k, v.as_str()) == want),
            "{want:?} in {facts:?}"
        );
    }
}

#[test]
fn a_comic_shows_its_first_page_and_counts_its_pages() {
    let dir = tempfile::tempdir().unwrap();
    let file = zip_at(
        dir.path(),
        "comic.cbz",
        &[("10.png", picture()), ("2.png", picture())],
    );
    let probed = probe(&file).unwrap();
    let peeked = peek(&probed.source, &probed.sniffed, &pane_budget());
    assert!(matches!(peeked.body, Body::Picture(_)));
    let facts = rows(&peeked.facts);
    assert!(
        facts.iter().any(|(k, v)| *k == "pages" && v == "2"),
        "{facts:?}"
    );
}

#[test]
fn a_broken_epub_still_shows_what_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let file = zip_at(
        dir.path(),
        "broken.epub",
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", b"<x".to_vec()),
        ],
    );
    let probed = probe(&file).unwrap();
    let peeked = peek(&probed.source, &probed.sniffed, &pane_budget());
    assert_eq!(peeked.body.slug(), "facts");
    assert!(
        rows(&peeked.facts)
            .iter()
            .any(|(k, v)| *k == "kind" && v == "Book (EPUB)")
    );
}
