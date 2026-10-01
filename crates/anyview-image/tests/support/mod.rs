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

/// What sniffing says of `bytes` called `name`.
pub fn sniffed(bytes: &[u8], name: &str) -> Sniffed {
    let head = &bytes[..bytes.len().min(4096)];
    match sniff(&FileHead::new(head), &FileName::new(name).unwrap()) {
        SniffStep::Done(sniffed) => sniffed,
        SniffStep::LookInside(_) => panic!("{name} is not a zip"),
    }
}

/// The fixture as the viewer would be handed it.
pub fn fixture(name: &str) -> (Source, Sniffed) {
    let bytes = bytes(name);
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    let source = Source::new(FilePath::new(path(name)).unwrap(), stamp);
    (source, sniffed(&bytes, name))
}

/// A budget of `pixels` pixels and room for as many as that at four bytes each.
pub fn budget(pixels: u64) -> PeekBudget {
    PeekBudget {
        bytes: ByteLen(pixels * 4),
        pixels: PixelArea(pixels),
        time: Duration::from_millis(500),
    }
}
