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
    let woff = dir.path().join("web.woff2");
    std::fs::write(&woff, b"wOF2\x00\x01\x00\x00restofheader").unwrap();
    let (src, sniffed) = opened(&woff, "web.woff2");
    let font = FontPeek::peek(&src, &sniffed, &budget(1 << 20)).unwrap();
    assert_eq!(font.format, FontFormat::Woff2);
    assert!(font.face.is_none());
    assert_eq!(
        FontPeek::facts(&font)
            .value(FactLabel::Kind)
            .map(|v| v.as_str()),
        Some("WOFF2 web font")
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

/// `ttf` wrapped as a WOFF: every table zlib-compressed where that makes it smaller.
fn woff_of(ttf: &[u8]) -> Vec<u8> {
    let count = u16::from_be_bytes([ttf[4], ttf[5]]) as usize;
    let mut entries = Vec::new();
    for index in 0..count {
        let at = 12 + 16 * index;
        let offset = u32::from_be_bytes(ttf[at + 8..at + 12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(ttf[at + 12..at + 16].try_into().unwrap()) as usize;
        let data = &ttf[offset..offset + length];
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
        let stored = if packed.len() < data.len() {
            packed
        } else {
            data.to_vec()
        };
        entries.push((&ttf[at..at + 4], &ttf[at + 4..at + 8], stored, length));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"wOFF");
    out.extend_from_slice(&ttf[..4]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(count as u16).to_be_bytes());
    out.extend_from_slice(&[0; 30]);
    let mut offset = 44 + 20 * count;
    let mut body = Vec::new();
    for (tag, checksum, stored, length) in &entries {
        out.extend_from_slice(tag);
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(stored.len() as u32).to_be_bytes());
        out.extend_from_slice(&(*length as u32).to_be_bytes());
        out.extend_from_slice(checksum);
        body.extend_from_slice(stored);
        let padding = (4 - stored.len() % 4) % 4;
        body.resize(body.len() + padding, 0);
        offset += stored.len() + padding;
    }
    out.extend_from_slice(&body);
    out
}

#[test]
fn a_woff_gives_the_same_face_as_the_font_it_wraps() {
    let ttf = std::fs::read(fixture("blocks.ttf")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blocks.woff");
    std::fs::write(&path, woff_of(&ttf)).unwrap();
    let (src, sniffed) = opened(&path, "blocks.woff");
    let woff = FontPeek::peek(&src, &sniffed, &budget(1 << 20)).unwrap();
    let plain = peeked("blocks.ttf");
    assert_eq!(woff.format, FontFormat::Woff);
    assert_eq!(woff.face, plain.face);
    assert!(woff.face.as_ref().unwrap().glyphs > 0);
}

#[test]
fn a_woff_with_a_table_that_does_not_unpack_is_malformed() {
    let ttf = std::fs::read(fixture("blocks.ttf")).unwrap();
    let mut woff = woff_of(&ttf);
    // Corrupt the first table's stored bytes.
    let offset = u32::from_be_bytes(woff[48..52].try_into().unwrap()) as usize;
    let stored = u32::from_be_bytes(woff[52..56].try_into().unwrap()) as usize;
    let original = u32::from_be_bytes(woff[56..60].try_into().unwrap()) as usize;
    assert!(stored < original, "the fixture's first table is compressed");
    woff[offset..offset + stored].fill(0xff);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.woff");
    std::fs::write(&path, woff).unwrap();
    let (src, sniffed) = opened(&path, "bad.woff");
    let error = FontPeek::peek(&src, &sniffed, &budget(1 << 20)).unwrap_err();
    assert!(matches!(error, FontError::Malformed { .. }), "{error:?}");
}
