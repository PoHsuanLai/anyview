//! Files in a scratch folder, as the viewer is handed them: a `Source` and what sniffing made of it.
// Each test crate uses some of these helpers, and none of them is a `#[test]` function.
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{
    ByteLen, FileHead, FilePath, FileStamp, ModTime, PixelLen, PixelSize, SniffStep, Sniffed,
    Source, sniff,
};
use anyview_export::{DocumentExport, export};
use anyview_image::Rgba8;
use std::path::{Path, PathBuf};

/// A fixture file's bytes.
pub fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).unwrap()
}

/// What the viewer knows of the file at `path`.
pub fn opened(path: &Path) -> (Source, Sniffed) {
    let bytes = std::fs::read(path).unwrap();
    let head = FileHead::new(&bytes[..bytes.len().min(4096)]);
    let name = FilePath::new(path).unwrap().file_name().unwrap();
    let SniffStep::Done(sniffed) = sniff(&head, &name) else {
        panic!("{path:?} is a zip");
    };
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    (Source::new(FilePath::new(path).unwrap(), stamp), sniffed)
}

/// `bytes` written as `name` in `dir`.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// A picture of one colour, `width` by `height`.
pub fn flat(width: u32, height: u32, rgba: [u8; 4]) -> Rgba8 {
    let size = PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    };
    let bytes = rgba.repeat((width * height) as usize);
    Rgba8::new(size, bytes).unwrap()
}

/// The files `choice` writes for the file at `path`.
pub fn exported(path: &Path, choice: DocumentExport) -> Vec<PathBuf> {
    let (source, sniffed) = opened(path);
    export(&source, &sniffed, choice)
        .unwrap()
        .iter()
        .map(|file| file.as_path().to_path_buf())
        .collect()
}

/// The names of the files in `dir`, sorted.
pub fn names(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// The width and height of the picture in `path`.
pub fn size_of(path: &Path) -> (u32, u32) {
    let (_, sniffed) = opened(path);
    let bytes = std::fs::read(path).unwrap();
    match anyview_image::decode_bytes(&bytes, &sniffed).unwrap() {
        anyview_image::Decoded::Still(picture) => {
            let size = picture.size();
            (size.width.0, size.height.0)
        }
        anyview_image::Decoded::Animated(_) | anyview_image::Decoded::HeldStill { .. } => {
            panic!("a still")
        }
    }
}
