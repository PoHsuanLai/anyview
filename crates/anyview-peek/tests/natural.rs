//! The size a window opens to is read from the start of the file: a picture's header, a movie's
//! header. Never a decode, and bounded whatever the file claims.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FilePath, PixelLen, PixelSize, QuarterTurn};
use anyview_peek::natural_size;
use std::time::{Duration, Instant};
use support::{Home, path};

fn size(width: u32, height: u32) -> Option<PixelSize> {
    Some(PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    })
}

fn natural(home: Home, name: &str) -> Option<PixelSize> {
    natural_size(&FilePath::new(path(home, name)).unwrap())
}

fn natural_at(file: &std::path::Path) -> Option<PixelSize> {
    natural_size(&FilePath::new(file).unwrap())
}

#[test]
fn each_raster_format_gives_its_header_size() {
    assert_eq!(natural(Home::Image, "quadrants.png"), size(48, 32));
    assert_eq!(natural(Home::Image, "plain.jpg"), size(48, 32));
    assert_eq!(natural(Home::Image, "lossy.webp"), size(48, 32));
    assert_eq!(natural(Home::Image, "anim.webp"), size(32, 24));
    assert_eq!(natural(Home::Image, "spin.gif"), size(32, 24));
}

#[test]
fn a_jpeg_turned_a_quarter_by_its_exif_swaps_its_sides() {
    let dir = tempfile::tempdir().unwrap();
    let plain = std::fs::read(path(Home::Image, "plain.jpg")).unwrap();
    for (turn, want) in [
        (QuarterTurn::None, size(48, 32)),
        (QuarterTurn::Quarter, size(32, 48)),
        (QuarterTurn::Half, size(48, 32)),
        (QuarterTurn::ThreeQuarter, size(32, 48)),
    ] {
        let file = dir.path().join(format!("turned-{turn:?}.jpg"));
        std::fs::write(&file, anyview_image::rotate_jpeg(&plain, turn).unwrap()).unwrap();
        assert_eq!(natural_at(&file), want, "{turn:?}");
    }
}

#[test]
fn an_svg_gives_the_size_it_declares() {
    assert_eq!(natural(Home::Image, "logo.svg"), size(64, 32));
}

#[test]
fn a_video_gives_its_resolution_from_the_header() {
    assert_eq!(natural(Home::Own, "clip.mp4"), size(64, 48));
    assert_eq!(natural(Home::Media, "clip.mkv"), size(64, 48));
}

#[test]
fn kinds_with_no_natural_size_give_none() {
    for (home, name) in [
        (Home::Own, "hello.pdf"),
        (Home::Own, "tone.wav"),
        (Home::Media, "cover.mp3"),
        (Home::Font, "blocks.ttf"),
    ] {
        assert_eq!(natural(home, name), None, "{name}");
    }
}

#[test]
fn a_folder_a_missing_file_and_an_empty_one_give_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(natural_at(dir.path()), None);
    assert_eq!(natural_at(&dir.path().join("gone.png")), None);
    for name in [
        "empty.png",
        "empty.jpg",
        "empty.svg",
        "empty.mp4",
        "empty.mkv",
    ] {
        std::fs::write(dir.path().join(name), b"").unwrap();
        assert_eq!(natural_at(&dir.path().join(name)), None, "{name}");
    }
}

#[test]
fn a_file_with_a_picture_extension_and_other_contents_gives_none() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.png");
    std::fs::write(&file, b"just some words, not a picture").unwrap();
    assert_eq!(natural_at(&file), None);
}

#[cfg(unix)]
#[test]
fn a_pipe_is_refused_rather_than_waited_on() {
    let dir = tempfile::tempdir().unwrap();
    let pipe = dir.path().join("pipe.png");
    let made = std::process::Command::new("mkfifo").arg(&pipe).status();
    if made.is_ok_and(|status| status.success()) {
        let began = Instant::now();
        assert_eq!(natural_at(&pipe), None);
        assert!(began.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn a_header_claiming_gigapixels_is_read_at_once_and_decodes_nothing() {
    let began = Instant::now();
    assert_eq!(
        natural(Home::Image, "bomb-canvas.gif"),
        size(65535, 65535),
        "the claim is passed on for the window's cap to meet"
    );
    assert_eq!(natural(Home::Image, "bomb-canvas.webp"), size(40000, 40000));
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "a header read, not a decode of what it claims"
    );
}

#[test]
fn a_drawing_that_asks_for_hours_of_filtering_is_sized_without_drawing() {
    let began = Instant::now();
    assert!(natural(Home::Image, "bomb-turbulence.svg").is_some());
    assert!(began.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_header_is_read_from_the_start_of_a_large_file_only() {
    // A real PNG header followed by far more than the bound of junk.
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = std::fs::read(path(Home::Image, "quadrants.png")).unwrap();
    bytes.resize(bytes.len() + 8 * 1024 * 1024, 0xAB);
    let file = dir.path().join("big.png");
    std::fs::write(&file, bytes).unwrap();
    let began = Instant::now();
    assert_eq!(natural_at(&file), size(48, 32));
    assert!(began.elapsed() < Duration::from_secs(5));
}
