//! A fake format drives `Peek` the way a generic consumer will.

use super::*;
use crate::facts::{FactLabel, FactValue, Facts};
use crate::kind::FormatKind;
use crate::sniff::{FileHead, SniffStep, Sniffed, sniff};
use crate::source::{ByteLen, FileName, FilePath, FileStamp, ModTime, Source};
use crate::units::PixelArea;
use ds_core::word::Word;
use std::time::Duration;

/// Peeks at plain text by counting the bytes the budget lets it read.
struct FakeText;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Counted(u64);

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("the budget allows no bytes")]
struct NoBudget;

impl Peek for FakeText {
    const KIND: FormatKind = FormatKind::PlainText;
    type Peeked = Counted;
    type Error = NoBudget;

    fn peek(src: &Source, _: &Sniffed, budget: &PeekBudget) -> Result<Counted, NoBudget> {
        match budget.bytes.0.min(src.stamp().len.0) {
            0 => Err(NoBudget),
            read => Ok(Counted(read)),
        }
    }

    fn facts(peeked: &Counted) -> Facts {
        Facts::empty().with(FactLabel::Size, FactValue::size(ByteLen(peeked.0)))
    }
}

/// What a generic consumer does with any format: peek, then list the facts.
fn peek_facts<P: Peek>(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Option<Facts> {
    (sniffed.kind() == P::KIND)
        .then(|| P::peek(src, sniffed, budget).ok())
        .flatten()
        .map(|peeked| P::facts(&peeked))
}

fn budget(bytes: u64) -> PeekBudget {
    PeekBudget {
        bytes: ByteLen(bytes),
        pixels: PixelArea(1_000_000),
        time: Duration::from_millis(100),
    }
}

fn text_file(len: u64) -> (Source, Sniffed) {
    let name = FileName::new("notes.txt").unwrap();
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(b"hello"), &name) else {
        panic!("text is answered from its head");
    };
    let stamp = FileStamp {
        len: ByteLen(len),
        modified: ModTime(0),
    };
    (
        Source::new(FilePath::new("/notes.txt").unwrap(), stamp),
        sniffed,
    )
}

#[test]
fn a_peek_stays_inside_its_budget() {
    // name, file length, byte budget, bytes read
    const CASES: &[(&str, u64, u64, Option<u64>)] = &[
        ("budget smaller than the file", 5_000, 1_000, Some(1_000)),
        ("file smaller than the budget", 300, 1_000, Some(300)),
        ("no budget", 5_000, 0, None),
        ("empty file", 0, 1_000, None),
    ];
    for (name, len, bytes, want) in CASES {
        let (src, sniffed) = text_file(*len);
        let got = FakeText::peek(&src, &sniffed, &budget(*bytes))
            .ok()
            .map(|c| c.0);
        assert_eq!(got, *want, "{name}");
    }
}

#[test]
fn a_generic_consumer_lists_the_facts_of_the_format_it_peeked() {
    let (src, sniffed) = text_file(2_000);
    let facts = peek_facts::<FakeText>(&src, &sniffed, &budget(1_500)).unwrap();
    assert_eq!(
        facts.value(FactLabel::Size).map(FactValue::as_str),
        Some("1.5 KB")
    );
    assert_eq!(peek_facts::<FakeText>(&src, &sniffed, &budget(0)), None);
}

#[test]
fn a_format_only_peeks_at_its_own_kind() {
    let name = FileName::new("a.png").unwrap();
    let SniffStep::Done(png) = sniff(
        &FileHead::new(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0"),
        &name,
    ) else {
        panic!("a png is answered from its head");
    };
    let (src, _) = text_file(100);
    assert_eq!(png.kind(), FormatKind::Raster);
    assert_eq!(peek_facts::<FakeText>(&src, &png, &budget(100)), None);
}

#[test]
fn stage_support_words() {
    assert_eq!(
        StageSupport::ALL,
        &[StageSupport::Stage, StageSupport::PeekOnly]
    );
}
