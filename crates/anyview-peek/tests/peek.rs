//! Peeking at the committed fixtures through the registry: the body each kind produces and the
//! facts it lists, against recorded expectations.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FormatKind;
use anyview_peek::{AnyPeeked, Body, peek};
use support::{Home, budget, fixture, on_disk, pane_budget, rows};

fn peeked(home: Home, name: &str) -> AnyPeeked {
    let (src, sniffed) = fixture(home, name);
    peek(&src, &sniffed, &pane_budget())
}

#[test]
fn every_fixture_peeks_into_the_body_of_its_kind() {
    // name, home, file, kind, body word
    const CASES: &[(&str, Home, &str, FormatKind, &str)] = &[
        (
            "png",
            Home::Image,
            "quadrants.png",
            FormatKind::Raster,
            "picture",
        ),
        (
            "jpeg",
            Home::Image,
            "plain.jpg",
            FormatKind::Raster,
            "picture",
        ),
        (
            "webp",
            Home::Image,
            "anim.webp",
            FormatKind::Raster,
            "picture",
        ),
        (
            "gif",
            Home::Image,
            "spin.gif",
            FormatKind::Raster,
            "picture",
        ),
        (
            "jxl",
            Home::Image,
            "photo.jxl",
            FormatKind::Raster,
            "picture",
        ),
        (
            "svg",
            Home::Image,
            "logo.svg",
            FormatKind::Vector,
            "picture",
        ),
        ("pdf", Home::Own, "hello.pdf", FormatKind::Pdf, "page"),
        (
            "text",
            Home::Text,
            "notes.txt",
            FormatKind::PlainText,
            "plain",
        ),
        ("code", Home::Text, "sample.rs", FormatKind::Code, "code"),
        (
            "markdown",
            Home::Text,
            "readme.md",
            FormatKind::Markdown,
            "markdown",
        ),
        ("csv", Home::Text, "people.csv", FormatKind::Table, "table"),
        ("tsv", Home::Text, "scores.tsv", FormatKind::Table, "table"),
        ("json", Home::Text, "config.json", FormatKind::Tree, "tree"),
        (
            "json lines",
            Home::Text,
            "events.jsonl",
            FormatKind::Tree,
            "tree",
        ),
    ];
    for (name, home, file, kind, body) in CASES {
        let peeked = peeked(*home, file);
        assert_eq!(peeked.kind, *kind, "{name}");
        assert_eq!(peeked.body.slug(), *body, "{name}");
        assert_eq!(peeked.name, *file, "{name}");
    }
}

#[test]
fn facts_list_the_formats_rows_then_the_size_and_the_date() {
    // name, home, file, rows
    type Case = (
        &'static str,
        Home,
        &'static str,
        &'static [(&'static str, &'static str)],
    );
    const CASES: &[Case] = &[
        (
            "png",
            Home::Image,
            "quadrants.png",
            &[
                ("kind", "PNG image"),
                ("dimensions", "48 × 32"),
                ("colour", "RGBA, 8-bit"),
                ("size", "122 B"),
                ("modified", "1970-01-01 00:00 UTC"),
            ],
        ),
        (
            "gif",
            Home::Image,
            "spin.gif",
            &[
                ("kind", "GIF image"),
                ("dimensions", "32 × 24"),
                ("frames", "3"),
                ("colour", "RGBA, 8-bit"),
                ("size", "282 B"),
                ("modified", "1970-01-01 00:00 UTC"),
            ],
        ),
        (
            "pdf",
            Home::Own,
            "hello.pdf",
            &[
                ("kind", "PDF document"),
                ("dimensions", "300 × 200 pt"),
                ("size", "621 B"),
                ("modified", "1970-01-01 00:00 UTC"),
            ],
        ),
        (
            "text",
            Home::Text,
            "notes.txt",
            &[
                ("kind", "Plain text"),
                ("lines", "60"),
                ("encoding", "UTF-8"),
                ("size", "2.2 KB"),
                ("modified", "1970-01-01 00:00 UTC"),
            ],
        ),
        (
            "table",
            Home::Text,
            "people.csv",
            &[
                ("kind", "CSV table"),
                ("rows", "50"),
                ("columns", "3"),
                ("encoding", "UTF-8"),
                ("size", "905 B"),
                ("modified", "1970-01-01 00:00 UTC"),
            ],
        ),
        (
            "json lines",
            Home::Text,
            "events.jsonl",
            &[
                ("kind", "JSON Lines"),
                ("entries", "100"),
                ("encoding", "UTF-8"),
                ("size", "2.5 KB"),
                ("modified", "1970-01-01 00:00 UTC"),
            ],
        ),
    ];
    for (name, home, file, want) in CASES {
        let want: Vec<(&str, String)> = want.iter().map(|(l, v)| (*l, (*v).to_owned())).collect();
        assert_eq!(rows(&peeked(*home, file).facts), want, "{name}");
    }
}

#[test]
fn the_text_bodies_hold_what_their_peeks_do() {
    let Body::Plain(plain) = peeked(Home::Text, "notes.txt").body else {
        panic!("plain text peeks to a Plain body");
    };
    assert_eq!(plain.lines.len(), 40);
    let Body::Table(table) = peeked(Home::Text, "people.csv").body else {
        panic!("a csv peeks to a Table body");
    };
    assert_eq!(table.rows.len(), 40);
    assert!(table.header.is_some());
    let Body::Picture(image) = peeked(Home::Image, "quadrants.png").body else {
        panic!("a png peeks to a Picture body");
    };
    assert_eq!(image.picture.size().width.0, 48);
}

#[test]
fn a_peek_that_fails_still_gives_the_files_own_facts() {
    let dir = tempfile::tempdir().unwrap();
    // A PNG signature and nothing else: sniffing says raster, the decoder says no.
    let broken = dir.path().join("broken.png");
    std::fs::write(&broken, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").unwrap();
    let (src, sniffed) = on_disk(&broken, 0);
    let peeked = peek(&src, &sniffed, &pane_budget());
    assert_eq!(peeked.kind, FormatKind::Raster);
    let Body::Unavailable(reason) = &peeked.body else {
        panic!("an undecodable png is unavailable, not a peek");
    };
    assert!(!reason.is_empty());
    let labels: Vec<&str> = rows(&peeked.facts)
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(labels, ["kind", "size", "modified"]);
}

#[test]
fn a_pdf_longer_than_the_byte_budget_is_not_read() {
    let (src, sniffed) = fixture(Home::Own, "hello.pdf");
    let peeked = peek(&src, &sniffed, &budget(100, 1_000_000));
    let Body::Unavailable(reason) = &peeked.body else {
        panic!("a pdf past the byte budget is unavailable");
    };
    assert_eq!(reason, "the file is 621 bytes and the preview may read 100");
    assert_eq!(
        rows(&peeked.facts)[0],
        ("kind", "application/pdf".to_owned())
    );
}

#[test]
fn a_damaged_pdf_is_a_page_that_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let damaged = dir.path().join("damaged.pdf");
    std::fs::write(&damaged, b"%PDF-1.4\nthis is not a pdf at all\n").unwrap();
    let (src, sniffed) = on_disk(&damaged, 0);
    let peeked = peek(&src, &sniffed, &pane_budget());
    assert_eq!(
        rows(&peeked.facts)[0],
        ("kind", "PDF, no preview".to_owned())
    );
    assert_eq!(peeked.body.slug(), "page");
}

#[test]
fn kinds_without_a_back_end_show_their_type_size_and_date() {
    // name, file name, bytes, kind, kind words
    const CASES: &[(&str, &str, &[u8], FormatKind, &str)] = &[
        (
            "font",
            "face.ttf",
            b"\0\x01\0\0\0\x0f\0\x80\0\x03\0\x30",
            FormatKind::Font,
            "Font (TTF)",
        ),
        (
            "archive",
            "bundle.zip",
            b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            FormatKind::Archive,
            "Archive (ZIP)",
        ),
        (
            "other",
            "blob.bin",
            b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0",
            FormatKind::Other,
            "application/octet-stream",
        ),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (name, file, bytes, kind, words) in CASES {
        let path = dir.path().join(file);
        std::fs::write(&path, bytes).unwrap();
        let (src, sniffed) = on_disk(&path, 1_790_951_400_000_000_000);
        let peeked = peek(&src, &sniffed, &pane_budget());
        assert_eq!(peeked.kind, *kind, "{name}");
        assert_eq!(peeked.body.slug(), "facts", "{name}");
        let want = vec![
            ("kind", (*words).to_owned()),
            ("size", format!("{} B", bytes.len())),
            ("modified", "2026-10-02 14:30 UTC".to_owned()),
        ];
        assert_eq!(rows(&peeked.facts), want, "{name}");
    }
}

#[test]
fn a_folder_counts_its_items_size_and_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, bytes: &[u8]| std::fs::write(dir.path().join(name), bytes).unwrap();
    write("a.txt", b"one\ntwo\n");
    write("b.txt", b"three\n");
    write(
        "c.png",
        &std::fs::read(support::path(Home::Image, "quadrants.png")).unwrap(),
    );
    write(".hidden", b"not counted");
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let stamp = anyview_core::FileStamp {
        len: anyview_core::ByteLen(0),
        modified: anyview_core::ModTime(0),
    };
    let src = anyview_core::Source::new(anyview_core::FilePath::new(dir.path()).unwrap(), stamp);
    let peeked = peek(&src, &anyview_core::sniff_folder(), &pane_budget());
    assert_eq!(peeked.kind, FormatKind::Folder);
    let Body::Folder(folder) = &peeked.body else {
        panic!("a folder peeks to a Folder body");
    };
    assert_eq!((folder.folders, folder.files), (1, 3));
    assert_eq!(folder.size, anyview_core::ByteLen(8 + 6 + 122));
    let want = vec![
        ("kind", "Folder".to_owned()),
        ("entries", "4 items: 2 plain text, 1 raster".to_owned()),
        ("size", "136 B".to_owned()),
        ("modified", "1970-01-01 00:00 UTC".to_owned()),
    ];
    assert_eq!(rows(&peeked.facts), want);
}

#[test]
fn a_folder_that_cannot_be_listed_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("gone");
    let stamp = anyview_core::FileStamp {
        len: anyview_core::ByteLen(0),
        modified: anyview_core::ModTime(0),
    };
    let src = anyview_core::Source::new(anyview_core::FilePath::new(&missing).unwrap(), stamp);
    let peeked = peek(&src, &anyview_core::sniff_folder(), &pane_budget());
    assert_eq!(peeked.body.slug(), "unavailable");
}
