//! Peeking at the committed fixtures through the registry: the body each kind produces and the
//! facts it lists, against recorded expectations.

use crate::support;

use anyview_core::FormatKind;
use anyview_peek::{AnyPeeked, Body, peek};
use support::{Home, budget, fixture, on_disk, pane_budget, rows};

fn peeked(home: Home, name: &str) -> AnyPeeked {
    let (src, sniffed) = fixture(home, name);
    peek(&src, &sniffed, &pane_budget())
}

#[test]
fn every_fixture_peeks_into_the_body_of_its_kind() {
    // name, home, file, kind, body word, a check on the body that the word does not make
    type Case = (
        &'static str,
        Home,
        &'static str,
        FormatKind,
        &'static str,
        Option<fn(&Body) -> bool>,
    );
    const CASES: &[Case] = &[
        (
            "png",
            Home::Image,
            "quadrants.png",
            FormatKind::Raster,
            "picture",
            Some(|body| matches!(body, Body::Picture(p) if p.picture.size().width.0 == 48)),
        ),
        (
            "jpeg",
            Home::Image,
            "plain.jpg",
            FormatKind::Raster,
            "picture",
            None,
        ),
        (
            "webp",
            Home::Image,
            "anim.webp",
            FormatKind::Raster,
            "picture",
            None,
        ),
        (
            "gif",
            Home::Image,
            "spin.gif",
            FormatKind::Raster,
            "picture",
            None,
        ),
        (
            "jxl",
            Home::Image,
            "photo.jxl",
            FormatKind::Raster,
            "picture",
            None,
        ),
        (
            "svg",
            Home::Image,
            "logo.svg",
            FormatKind::Vector,
            "picture",
            None,
        ),
        ("pdf", Home::Own, "hello.pdf", FormatKind::Pdf, "page", None),
        (
            "font",
            Home::Font,
            "blocks.ttf",
            FormatKind::Font,
            "font",
            None,
        ),
        (
            "text",
            Home::Text,
            "notes.txt",
            FormatKind::PlainText,
            "plain",
            Some(|body| matches!(body, Body::Plain(p) if p.lines.len() == 40)),
        ),
        (
            "code",
            Home::Text,
            "sample.rs",
            FormatKind::Code,
            "code",
            None,
        ),
        (
            "markdown",
            Home::Text,
            "readme.md",
            FormatKind::Markdown,
            "markdown",
            None,
        ),
        (
            "csv",
            Home::Text,
            "people.csv",
            FormatKind::Table,
            "table",
            Some(|body| matches!(body, Body::Table(t) if t.header.is_some() && t.rows.len() == 40)),
        ),
        (
            "tsv",
            Home::Text,
            "scores.tsv",
            FormatKind::Table,
            "table",
            None,
        ),
        (
            "json",
            Home::Text,
            "config.json",
            FormatKind::Tree,
            "tree",
            None,
        ),
        (
            "json lines",
            Home::Text,
            "events.jsonl",
            FormatKind::Tree,
            "tree",
            None,
        ),
    ];
    for (name, home, file, kind, body, check) in CASES {
        let peeked = peeked(*home, file);
        assert_eq!(peeked.kind, *kind, "{name}");
        assert_eq!(peeked.body.slug(), *body, "{name}");
        assert_eq!(peeked.name, *file, "{name}");
        if let Some(check) = check {
            // The picture is 48 wide, a table has a header and 40 rows, plain text has 40 lines.
            assert!(
                check(&peeked.body),
                "{name}: the body holds what its peek does"
            );
        }
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
                ("modified", "1 Jan 1970 at 00:00"),
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
                ("modified", "1 Jan 1970 at 00:00"),
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
                ("modified", "1 Jan 1970 at 00:00"),
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
                ("modified", "1 Jan 1970 at 00:00"),
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
                ("modified", "1 Jan 1970 at 00:00"),
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
                ("modified", "1 Jan 1970 at 00:00"),
            ],
        ),
    ];
    for (name, home, file, want) in CASES {
        let want: Vec<(&str, String)> = want.iter().map(|(l, v)| (*l, (*v).to_owned())).collect();
        assert_eq!(rows(&peeked(*home, file).facts), want, "{name}");
    }
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
    const CASES: &[(&str, &str, &[u8], FormatKind, &str)] = &[(
        "other",
        "blob.bin",
        b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0",
        FormatKind::Other,
        "application/octet-stream",
    )];
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
            ("modified", "2 Oct 2026 at 14:30".to_owned()),
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
        ("modified", "1 Jan 1970 at 00:00".to_owned()),
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

#[test]
fn a_font_lists_its_names_and_glyph_count_then_the_size_and_the_date() {
    let peeked = peeked(Home::Font, "blocks.ttf");
    let rows = rows(&peeked.facts);
    let labels: Vec<&str> = rows.iter().map(|(label, _)| *label).collect();
    assert_eq!(
        labels,
        ["kind", "family", "style", "glyphs", "size", "modified"]
    );
    assert_eq!(rows[0].1, "TrueType font");
    assert_eq!(rows[1].1, "Anyview Blocks");
    assert_eq!(rows[2].1, "Regular");
    let Body::Font(font) = &peeked.body else {
        panic!("a font peeks to a Font body, not {}", peeked.body.slug());
    };
    assert_eq!(
        font.face.as_ref().map(|face| face.specimen.lines.len()),
        Some(3)
    );
}

#[test]
fn an_archive_lists_its_entries_then_the_size_and_the_date() {
    let dir = tempfile::tempdir().unwrap();
    let (src, sniffed) = support::zip_on_disk(
        dir.path(),
        &[
            ("docs/", ""),
            ("docs/readme.txt", "hello archive"),
            ("data.bin", "0123456789"),
        ],
    );
    let peeked = peek(&src, &sniffed, &pane_budget());
    assert_eq!(peeked.kind, FormatKind::Archive);
    let Body::Archive(archive) = &peeked.body else {
        panic!("a zip peeks to an Archive body, not {}", peeked.body.slug());
    };
    let names: Vec<&str> = archive
        .listing
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(names, ["docs/", "docs/readme.txt", "data.bin"]);
    let facts = rows(&peeked.facts);
    assert_eq!(facts[0], ("kind", "ZIP archive".to_owned()));
    assert_eq!(facts[1], ("entries", "3, 23 B unpacked".to_owned()));
    assert_eq!(facts[2].0, "size");
    assert_eq!(facts[3], ("modified", "2 Oct 2026 at 14:30".to_owned()));
}

#[test]
fn a_damaged_archive_is_unavailable_with_its_kind_and_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.zip");
    std::fs::write(
        &path,
        b"PK\x03\x04 and then the file simply stops being a zip",
    )
    .unwrap();
    let (src, sniffed) = on_disk(&path, 0);
    let peeked = peek(&src, &sniffed, &pane_budget());
    let Body::Unavailable(reason) = &peeked.body else {
        panic!("a broken zip is unavailable, not {}", peeked.body.slug());
    };
    assert!(reason.starts_with("not a valid ZIP archive"), "{reason}");
    assert_eq!(rows(&peeked.facts)[0], ("kind", "Archive (ZIP)".to_owned()));
}
