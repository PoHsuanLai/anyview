//! Loading fixtures the way the viewer meets files: a `Source` on disk and what sniffing made of it.
//! The image and text fixtures are the back-end crates' own; this crate adds a PDF.
// Each test crate uses some of these helpers, and none of them is a `#[test]` function.
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{
    ByteLen, FileHead, FileName, FilePath, FileStamp, ModTime, PeekBudget, PixelArea, SniffStep,
    Sniffed, Source, ZipEntries, sniff, sniff_zip,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where a fixture lives: `own` (this crate's), or a back end's.
#[derive(Debug, Clone, Copy)]
pub enum Home {
    Own,
    Image,
    Text,
}

/// The path of a fixture.
pub fn path(home: Home, name: &str) -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let dir = match home {
        Home::Own => Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf(),
        Home::Image => crates.join("anyview-image"),
        Home::Text => crates.join("anyview-text"),
    };
    dir.join("tests/fixtures").join(name)
}

/// What sniffing says of `bytes` called `name`; a zip is an archive (nothing here opens one).
pub fn sniffed(bytes: &[u8], name: &str) -> Sniffed {
    let head = &bytes[..bytes.len().min(4096)];
    match sniff(&FileHead::new(head), &FileName::new(name).unwrap()) {
        SniffStep::Done(sniffed) => sniffed,
        SniffStep::LookInside(probe) => {
            sniff_zip(probe, &ZipEntries::new(Vec::<String>::new(), None))
        }
    }
}

/// A file on disk as the viewer would be handed it, modified at `modified` nanoseconds.
pub fn on_disk(path: &Path, modified: i64) -> (Source, Sniffed) {
    let bytes = std::fs::read(path).unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(modified),
    };
    (
        Source::new(FilePath::new(path).unwrap(), stamp),
        sniffed(&bytes, name),
    )
}

/// The fixture as the viewer would be handed it.
pub fn fixture(home: Home, name: &str) -> (Source, Sniffed) {
    on_disk(&path(home, name), 0)
}

/// A budget that reads `bytes` and decodes `pixels`.
pub fn budget(bytes: u64, pixels: u64) -> PeekBudget {
    PeekBudget {
        bytes: ByteLen(bytes),
        pixels: PixelArea(pixels),
        time: Duration::from_millis(500),
    }
}

/// What the launcher would allow: plenty of bytes, a pane's worth of pixels.
pub fn pane_budget() -> PeekBudget {
    budget(4_000_000, 1_000_000)
}

/// The facts as `(label slug, text)` rows, in order.
pub fn rows(facts: &anyview_core::Facts) -> Vec<(&'static str, String)> {
    use ds::prelude::Word;
    facts
        .rows()
        .iter()
        .map(|fact| (fact.label.slug(), fact.value.as_str().to_owned()))
        .collect()
}
