//! The font peek through the public API, on a real face and on faces built from it.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use anyview_core::{
    ByteLen, FactLabel, FileHead, FileName, FilePath, FileStamp, FontFormat, FormatKind, ModTime,
    Peek, PeekBudget, PixelArea, SniffStep, Sniffed, Source, sniff,
};
use anyview_font::{EM, FontError, FontPeek, FontPeeked, Variation};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn budget(bytes: u64) -> PeekBudget {
    PeekBudget {
        bytes: ByteLen(bytes),
        pixels: PixelArea(1 << 20),
        time: Duration::from_secs(1),
    }
}

/// `path` as the viewer would be handed it, named `name` for sniffing.
fn opened(path: &Path, name: &str) -> (Source, Sniffed) {
    let bytes = std::fs::read(path).unwrap();
    let head = FileHead::new(&bytes[..bytes.len().min(4096)]);
    let SniffStep::Done(sniffed) = sniff(&head, &FileName::new(name).unwrap()) else {
        panic!("a font head is answered at once");
    };
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    (Source::new(FilePath::new(path).unwrap(), stamp), sniffed)
}

fn peeked(name: &str) -> FontPeeked {
    let (src, sniffed) = opened(&fixture(name), name);
    FontPeek::peek(&src, &sniffed, &budget(1 << 20)).unwrap()
}

#[test]
fn a_truetype_face_gives_its_names_its_glyph_count_and_a_specimen() {
    let font = peeked("blocks.ttf");
    let face = font.face.as_ref().unwrap();
    assert_eq!(font.format, FontFormat::Ttf);
    assert_eq!(font.faces, 1);
    assert_eq!(face.family, "Anyview Blocks");
    assert_eq!(face.style, "Regular");
    assert_eq!(face.variable, Variation::Fixed);
    assert_eq!(face.glyphs, 68); // the 67 sample characters and .notdef
    // The three sample lines, each a drawn row of outlines in the common em.
    let texts: Vec<&str> = face
        .specimen
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(texts, ["ABCDEFGHIJKLM", "abcdefghijklm", "0123456789 &?!@"]);
    for line in &face.specimen.lines {
        assert!(
            line.path.starts_with('M') && line.path.ends_with('Z'),
            "{}",
            line.text
        );
        assert!(
            line.height > EM / 2 && line.height < EM * 2,
            "{}",
            line.height
        );
        // Thirteen capitals at about 0.6 em each.
        assert!(line.width > EM * 5, "{} wide", line.width);
    }
}

#[test]
fn the_facts_name_the_face() {
    let facts = FontPeek::facts(&peeked("blocks.ttf"));
    let rows: Vec<(FactLabel, &str)> = facts
        .rows()
        .iter()
        .map(|row| (row.label, row.value.as_str()))
        .collect();
    assert_eq!(rows[0], (FactLabel::Kind, "TrueType font"));
    assert_eq!(rows[1], (FactLabel::Family, "Anyview Blocks"));
    assert_eq!(rows[2], (FactLabel::Style, "Regular"));
    assert_eq!(rows[3].0, FactLabel::Glyphs);
}

#[test]
fn a_font_with_none_of_the_sample_letters_shows_the_characters_it_has() {
    let font = peeked("circled.ttf");
    let face = font.face.unwrap();
    let texts: Vec<&str> = face
        .specimen
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(
        texts,
        ["\u{2460}\u{2461}\u{2462}\u{2463}\u{2464}\u{2465}\u{2466}\u{2467}\u{2468}\u{2469}"]
    );
}

/// A collection of two faces that are both Blocks: the directory copied behind a `ttcf` header,
/// its table offsets moved by the header's length.
fn collection(dir: &Path) -> PathBuf {
    let font = std::fs::read(fixture("blocks.ttf")).unwrap();
    let header = 20u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"ttcf");
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&header.to_be_bytes());
    out.extend_from_slice(&header.to_be_bytes());
    let mut body = font.clone();
    let tables = u16::from_be_bytes([body[4], body[5]]) as usize;
    for table in 0..tables {
        let at = 12 + table * 16 + 8;
        let offset = u32::from_be_bytes(body[at..at + 4].try_into().unwrap()) + header;
        body[at..at + 4].copy_from_slice(&offset.to_be_bytes());
    }
    out.extend_from_slice(&body);
    let path = dir.join("pair.ttc");
    std::fs::write(&path, out).unwrap();
    path
}

#[test]
fn a_collection_reports_its_faces_and_opens_the_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = collection(dir.path());
    let (src, sniffed) = opened(&path, "pair.ttc");
    assert_eq!(sniffed.kind(), FormatKind::Font);
    let font = FontPeek::peek(&src, &sniffed, &budget(1 << 20)).unwrap();
    assert_eq!(font.format, FontFormat::Ttc);
    assert_eq!(font.faces, 2);
    assert_eq!(font.face.as_ref().unwrap().family, "Anyview Blocks");
    let facts = FontPeek::facts(&font);
    assert_eq!(
        facts.value(FactLabel::Kind).map(|v| v.as_str()),
        Some("Font collection, 2 faces")
    );
}

#[test]
fn a_font_longer_than_the_budget_is_refused_and_a_web_font_is_named_not_opened() {
    let (src, sniffed) = opened(&fixture("blocks.ttf"), "blocks.ttf");
    let len = src.stamp().len;
    assert_eq!(
        FontPeek::peek(&src, &sniffed, &budget(1000)),
        Err(FontError::OverBudget {
            len,
            allowed: ByteLen(1000)
        })
    );
    let dir = tempfile::tempdir().unwrap();
    let woff = dir.path().join("web.woff");
    std::fs::write(&woff, b"wOFF\x00\x01\x00\x00restofheader").unwrap();
    let (src, sniffed) = opened(&woff, "web.woff");
    let font = FontPeek::peek(&src, &sniffed, &budget(1 << 20)).unwrap();
    assert_eq!(font.format, FontFormat::Woff);
    assert!(font.face.is_none());
    assert_eq!(
        FontPeek::facts(&font)
            .value(FactLabel::Kind)
            .map(|v| v.as_str()),
        Some("WOFF web font")
    );
}

#[test]
fn bytes_that_are_not_a_font_are_malformed() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.ttf");
    std::fs::write(&bad, b"\x00\x01\x00\x00 and then nothing a font would hold").unwrap();
    let (src, sniffed) = opened(&bad, "bad.ttf");
    let got = FontPeek::peek(&src, &sniffed, &budget(1 << 20));
    assert!(matches!(got, Err(FontError::Malformed { .. })), "{got:?}");
}
