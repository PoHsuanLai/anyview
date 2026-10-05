//! Peeking at office documents: a document with a thumbnail shows the thumbnail, one without shows
//! its facts, and a spreadsheet is a table the peek leaves to the viewer.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FilePath, FormatKind};
use anyview_peek::{Body, peek, probe};
use std::io::Write;
use std::path::Path;
use support::{pane_budget, rows};

/// A 1x1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

const CORE: &[u8] = br#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="c" xmlns:dc="d"><dc:title>Plans</dc:title><dc:creator>Ann</dc:creator></cp:coreProperties>"#;

fn docx(dir: &Path, extra: &[(&str, &[u8])]) -> FilePath {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    let base: [(&str, &[u8]); 3] = [
        ("[Content_Types].xml", b"<Types/>"),
        ("word/document.xml", b"<w/>"),
        ("docProps/core.xml", CORE),
    ];
    for (name, bytes) in base.iter().chain(extra) {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    let file = dir.join("plans.docx");
    std::fs::write(&file, writer.finish().unwrap().into_inner()).unwrap();
    FilePath::new(file).unwrap()
}

#[test]
fn a_document_with_a_thumbnail_shows_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = docx(dir.path(), &[("docProps/thumbnail.png", PNG)]);
    let probed = probe(&path).unwrap();
    assert_eq!(probed.sniffed.kind(), FormatKind::Office);
    let peeked = peek(&probed.source, &probed.sniffed, &pane_budget());
    assert!(matches!(peeked.body, Body::Picture(_)), "{:?}", peeked.body);
    let facts = rows(&peeked.facts);
    assert!(facts.contains(&("title", "Plans".to_owned())), "{facts:?}");
    assert!(facts.contains(&("author", "Ann".to_owned())), "{facts:?}");
}

#[test]
fn a_document_without_a_thumbnail_shows_its_facts() {
    let dir = tempfile::tempdir().unwrap();
    let path = docx(dir.path(), &[]);
    let probed = probe(&path).unwrap();
    let peeked = peek(&probed.source, &probed.sniffed, &pane_budget());
    assert!(
        matches!(peeked.body, Body::FactsOnly(_)),
        "{:?}",
        peeked.body
    );
    assert!(
        rows(&peeked.facts).contains(&("title", "Plans".to_owned())),
        "the facts still list the title"
    );
}
