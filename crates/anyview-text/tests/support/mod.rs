//! Loading fixtures the way the viewer meets files: a `Source` on disk and what sniffing made of it.
// Each test crate uses some of these helpers, and none of them is a `#[test]` function.
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{
    ByteLen, FileHead, FileName, FilePath, FileStamp, ModTime, PeekBudget, PixelArea, SniffStep,
    Sniffed, Source, sniff,
};
use std::path::PathBuf;
use std::time::Duration;

/// The path of a fixture.
pub fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A fixture's bytes.
pub fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(path(name)).unwrap()
}

/// The fixture as the viewer would be handed it.
pub fn fixture(name: &str) -> (Source, Sniffed) {
    let bytes = bytes(name);
    let head = &bytes[..bytes.len().min(4096)];
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(head), &FileName::new(name).unwrap())
    else {
        panic!("{name} is not a zip");
    };
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    (
        Source::new(FilePath::new(path(name)).unwrap(), stamp),
        sniffed,
    )
}

/// A budget that reads `bytes` bytes.
pub fn budget(bytes: u64) -> PeekBudget {
    PeekBudget {
        bytes: ByteLen(bytes),
        pixels: PixelArea(0),
        time: Duration::from_millis(500),
    }
}

/// The facts as `(label slug, text)` rows, in order.
pub fn rows(facts: &anyview_core::Facts) -> Vec<(&'static str, String)> {
    use ds_core::word::Word;
    facts
        .rows()
        .iter()
        .map(|fact| (fact.label.slug(), fact.value.as_str().to_owned()))
        .collect()
}

/// `rows` written as a literal.
pub fn expected(rows: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    rows.iter().map(|(l, v)| (*l, (*v).to_owned())).collect()
}
