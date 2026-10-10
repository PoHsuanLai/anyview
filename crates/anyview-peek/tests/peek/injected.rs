//! Bytes a host injects (an attachment blob, a spool, an archive entry) are probed and peeked at
//! exactly as the file at a path is, inside the same budgets, and what needs a path refuses
//! cleanly.

use crate::support;

use anyview_core::{
    ByteLen, FactLabel, FileName, FilePath, FileStamp, FormatKind, Input, ModTime, ReadAt,
};
use anyview_fs::OnDisk;
use anyview_peek::{AnyPeeked, Body, PeekError, Unavailable, peek, probe};
use std::io::Write;
use std::sync::Arc;
use support::{Home, budget, pane_budget, path};

/// What the file at `file` peeks to, read as a path.
fn through_path(file: &std::path::Path) -> AnyPeeked {
    let probed = probe(FilePath::new(file).unwrap().on_disk()).unwrap();
    peek(&probed.input, &probed.sniffed, &pane_budget())
}

/// What the same bytes peek to when a host hands them in, called `name`.
fn through_bytes(name: &str, bytes: Vec<u8>) -> AnyPeeked {
    let probed = probe(Input::from((FileName::new(name).unwrap(), bytes))).unwrap();
    peek(&probed.input, &probed.sniffed, &pane_budget())
}

/// A card without its date, which a path has and bytes with no clock do not.
fn undated(card: &AnyPeeked) -> Vec<(String, String)> {
    card.facts
        .rows()
        .iter()
        .filter(|row| row.label != FactLabel::Modified)
        .map(|row| (format!("{:?}", row.label), row.value.as_str().to_owned()))
        .collect()
}

#[test]
fn injected_bytes_peek_as_the_file_at_a_path_does() {
    let fixtures = [
        (Home::Text, "notes.txt"),
        (Home::Text, "people.csv"),
        (Home::Text, "config.json"),
        (Home::Text, "sample.rs"),
        (Home::Text, "readme.md"),
        (Home::Image, "quadrants.png"),
        (Home::Image, "logo.svg"),
        (Home::Font, "blocks.ttf"),
        (Home::Media, "cover.mp3"),
        (Home::Media, "clip.mkv"),
        (Home::Own, "clip.mp4"),
        (Home::Own, "tone.wav"),
    ];
    for (home, name) in fixtures {
        let file = path(home, name);
        let from_path = through_path(&file);
        let from_bytes = through_bytes(name, std::fs::read(&file).unwrap());
        assert_eq!(from_bytes.kind, from_path.kind, "{name}: kind");
        assert_eq!(from_bytes.body, from_path.body, "{name}: body");
        assert_eq!(undated(&from_bytes), undated(&from_path), "{name}: facts");
    }
}

/// A zip of `entries` (name, bytes) as the bytes of a file.
fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (entry, bytes) in entries {
        writer.start_file(*entry, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn injected_zips_are_told_apart_and_listed() {
    let docx = zip_bytes(&[
        ("[Content_Types].xml", b"<Types/>"),
        ("word/document.xml", b"<w/>"),
        (
            "docProps/core.xml",
            br#"<cp:coreProperties xmlns:cp="c" xmlns:dc="d"><dc:title>Plans</dc:title></cp:coreProperties>"#,
        ),
    ]);
    let card = through_bytes("plans.docx", docx);
    assert_eq!(card.kind, FormatKind::Office);
    assert!(
        card.facts
            .rows()
            .iter()
            .any(|row| row.value.as_str() == "Plans"),
        "{:?}",
        card.facts
    );

    let archive = through_bytes(
        "bundle.zip",
        zip_bytes(&[("a.txt", b"one"), ("b.txt", b"two")]),
    );
    assert_eq!(archive.kind, FormatKind::Archive);
    let Body::Archive(listing) = &archive.body else {
        panic!("{:?}", archive.body);
    };
    assert_eq!(listing.listing.entries.len(), 2);

    let epub = zip_bytes(&[
        ("mimetype", b"application/epub+zip"),
        (
            "META-INF/container.xml",
            br#"<container><rootfiles><rootfile full-path="b.opf"/></rootfiles></container>"#,
        ),
        (
            "b.opf",
            br#"<package><metadata xmlns:dc="d"><dc:title>Story</dc:title></metadata><manifest><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c"/></spine></package>"#,
        ),
        ("c.xhtml", b"<html><body>hi</body></html>"),
    ]);
    let book = through_bytes("story.epub", epub);
    assert_eq!(book.kind, FormatKind::Book);
}

#[cfg(feature = "pane")]
#[test]
fn an_injected_pdf_is_rasterised_from_memory() {
    let bytes = std::fs::read(path(Home::Own, "hello.pdf")).unwrap();
    let card = through_bytes("hello.pdf", bytes);
    assert_eq!(card.kind, FormatKind::Pdf);
    assert!(matches!(card.body, Body::Page(_)), "{:?}", card.body);
}

#[test]
fn an_injected_folder_has_no_directory_to_list() {
    let probed = probe(
        FilePath::new(tempfile::tempdir().unwrap().path())
            .unwrap()
            .on_disk(),
    )
    .unwrap();
    assert_eq!(probed.sniffed.kind(), FormatKind::Folder);
    // The same sniffed answer for bytes that have no directory behind them is refused, not read.
    let injected = Input::from((FileName::new("dir").unwrap(), Vec::new()));
    let card = peek(&injected, &probed.sniffed, &pane_budget());
    assert!(matches!(card.body, Body::Unavailable(_)), "{:?}", card.body);
}

/// A source that claims to be small and is not: its stamp says one byte.
fn understated(name: &str, bytes: Vec<u8>) -> Input {
    let stamp = FileStamp {
        len: ByteLen(1),
        modified: ModTime(0),
    };
    Input::new(FileName::new(name).unwrap(), stamp, Arc::new(bytes))
}

#[test]
fn the_byte_budget_holds_for_injected_bytes_whatever_they_claim() {
    let png = std::fs::read(path(Home::Image, "quadrants.png")).unwrap();
    let font = std::fs::read(path(Home::Font, "blocks.ttf")).unwrap();
    for (name, bytes) in [("quadrants.png", png), ("blocks.ttf", font)] {
        let input = understated(name, bytes);
        let probed = probe(&input).unwrap();
        let card = peek(&probed.input, &probed.sniffed, &budget(64, 1_000_000));
        let Body::Unavailable(reason) = &card.body else {
            panic!("{name}: {:?}", card.body);
        };
        assert!(
            matches!(reason, Unavailable::TooBig { .. }),
            "{name}: {reason:?}"
        );
    }
}

/// A source whose reads fail, as a blob in a database that was deleted under the peek.
#[derive(Debug)]
struct Gone;

impl ReadAt for Gone {
    fn len(&self) -> ByteLen {
        ByteLen(10)
    }

    fn read_at(&self, _: u64, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::PermissionDenied.into())
    }
}

#[test]
fn a_source_that_cannot_be_read_is_unreadable_not_a_panic() {
    let stamp = FileStamp {
        len: ByteLen(10),
        modified: ModTime(0),
    };
    let input = Input::new(FileName::new("blob.png").unwrap(), stamp, Arc::new(Gone));
    let error = probe(&input).unwrap_err();
    assert!(
        matches!(
            error,
            PeekError::Unreadable {
                kind: std::io::ErrorKind::PermissionDenied,
                ..
            }
        ),
        "{error:?}"
    );
}
