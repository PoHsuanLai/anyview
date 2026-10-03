//! Peeking at recordings through the registry: libav's facts, and the cover an audio file carries.

#![cfg(feature = "media")]
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FormatKind;
use anyview_peek::{Body, peek};
use support::{Home, fixture, pane_budget, rows};

fn peeked(name: &str) -> anyview_peek::AnyPeeked {
    let (src, sniffed) = fixture(Home::Media, name);
    peek(&src, &sniffed, &pane_budget())
}

#[test]
fn a_video_lists_what_libav_read_and_draws_no_picture() {
    let video = peeked("clip.mkv");
    assert_eq!(video.kind, FormatKind::Video);
    assert_eq!(video.body.slug(), "facts");
    let rows = rows(&video.facts);
    let slugs: Vec<&str> = rows.iter().map(|(slug, _)| *slug).collect();
    assert_eq!(
        slugs,
        vec![
            "kind",
            "duration",
            "dimensions",
            "codec",
            "audio-codec",
            "size",
            "modified"
        ]
    );
    assert!(rows[0].1.starts_with("Video ("), "{}", rows[0].1);
    assert_eq!(rows[1].1, "0:03");
    assert_eq!(rows[2].1, "64 × 48");
    assert_eq!(rows[3].1, "mpeg4");
    assert_eq!(rows[4].1, "vorbis");
}

#[test]
fn an_audio_file_with_a_cover_shows_the_cover_reduced_to_the_budget() {
    let audio = peeked("cover.mp3");
    assert_eq!(audio.kind, FormatKind::Audio);
    let Body::Picture(cover) = &audio.body else {
        panic!("expected the cover, got {}", audio.body.slug());
    };
    assert_eq!(
        (cover.source_size.width.0, cover.source_size.height.0),
        (64, 64)
    );
    assert_eq!(
        cover.picture.size(),
        cover.source_size,
        "a small cover is not enlarged"
    );
    let rows = rows(&audio.facts);
    assert_eq!(rows[0].1, "Audio (MP3)");
    assert_eq!(rows[1], ("duration", "0:02".to_owned()));
    assert_eq!(rows[2], ("codec", "mp3".to_owned()));
}

#[test]
fn a_cover_larger_than_the_pixel_budget_is_reduced() {
    let (src, sniffed) = fixture(Home::Media, "cover.mp3");
    let small = support::budget(4_000_000, 256);
    let audio = peek(&src, &sniffed, &small);
    let Body::Picture(cover) = &audio.body else {
        panic!("expected the cover");
    };
    assert!(
        cover.picture.size().area().0 <= 256,
        "{:?}",
        cover.picture.size()
    );
    assert_eq!(cover.source_size.width.0, 64);
}

#[test]
fn audio_with_no_cover_is_facts_only_and_a_broken_file_says_why() {
    let flac = peeked("tone.flac");
    assert_eq!(flac.body.slug(), "facts");
    assert_eq!(rows(&flac.facts)[1], ("duration", "0:02".to_owned()));
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.mp3");
    std::fs::write(&broken, vec![0_u8; 64]).unwrap();
    let (src, sniffed) = support::on_disk(&broken, 0);
    let result = peek(&src, &sniffed, &pane_budget());
    assert_eq!(result.body.slug(), "unavailable");
}
