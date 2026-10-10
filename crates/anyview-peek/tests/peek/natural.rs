//! The size a window opens to is read from the start of the file: a picture's header, a movie's
//! header. Never a decode, and bounded whatever the file claims.

use crate::support;

use anyview_core::{FilePath, PixelLen, PixelSize, QuarterTurn};
use anyview_fs::OnDisk;
use anyview_peek::{is_audio, natural_size};
use std::time::{Duration, Instant};
use support::{Home, path};

fn size(width: u32, height: u32) -> Option<PixelSize> {
    Some(PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    })
}

fn natural(home: Home, name: &str) -> Option<PixelSize> {
    natural_size(FilePath::new(path(home, name)).unwrap().on_disk())
}

fn natural_at(file: &std::path::Path) -> Option<PixelSize> {
    natural_size(FilePath::new(file).unwrap().on_disk())
}

/// Where a row's file comes from: a shared fixture, or bytes written under the row's name.
enum Of {
    Fixture(Home, &'static str),
    Written(&'static str, &'static [u8]),
}

/// The size a file's start gives, row by row: (row, the file, the size or none).
#[test]
fn each_kind_gives_its_header_size_or_none() {
    let cases: [(&str, Of, Option<PixelSize>); 13] = [
        (
            "png",
            Of::Fixture(Home::Image, "quadrants.png"),
            size(48, 32),
        ),
        ("jpeg", Of::Fixture(Home::Image, "plain.jpg"), size(48, 32)),
        ("webp", Of::Fixture(Home::Image, "lossy.webp"), size(48, 32)),
        (
            "animated webp",
            Of::Fixture(Home::Image, "anim.webp"),
            size(32, 24),
        ),
        ("gif", Of::Fixture(Home::Image, "spin.gif"), size(32, 24)),
        ("svg", Of::Fixture(Home::Image, "logo.svg"), size(64, 32)),
        ("mp4", Of::Fixture(Home::Own, "clip.mp4"), size(64, 48)),
        ("mkv", Of::Fixture(Home::Media, "clip.mkv"), size(64, 48)),
        ("pdf has none", Of::Fixture(Home::Own, "hello.pdf"), None),
        ("wav has none", Of::Fixture(Home::Own, "tone.wav"), None),
        ("mp3 has none", Of::Fixture(Home::Media, "cover.mp3"), None),
        ("ttf has none", Of::Fixture(Home::Font, "blocks.ttf"), None),
        (
            "a picture extension on other contents",
            Of::Written("notes.png", b"just some words, not a picture"),
            None,
        ),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (row, of, want) in cases {
        let got = match of {
            Of::Fixture(home, name) => natural(home, name),
            Of::Written(name, bytes) => {
                let file = dir.path().join(name);
                std::fs::write(&file, bytes).unwrap();
                natural_at(&file)
            }
        };
        assert_eq!(got, want, "row {row}");
    }
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

#[test]
fn only_audio_files_are_audio() {
    let is = |home, name: &str| is_audio(FilePath::new(path(home, name)).unwrap().on_disk());
    for name in ["art.mp3", "art.flac", "art.m4a", "art.ogg", "plain.mp3"] {
        assert!(is(Home::Own, &format!("audio/{name}")), "{name}");
    }
    assert!(!is(Home::Own, "hello.pdf"));
    assert!(!is(Home::Own, "clip.mp4"));
    let dir = tempfile::tempdir().unwrap();
    assert!(!is_audio(FilePath::new(dir.path()).unwrap().on_disk()));
}
