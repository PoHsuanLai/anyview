//! The host's source of a video's frame: a cached thumbnail stands in for the picture a launcher
//! cannot decode, and only for a video that shows nothing else.

#![cfg(feature = "media")]
#![allow(clippy::unwrap_used)]

mod support;

use anyview_image::Rgba8;
use anyview_peek::{Body, NoFrames, VideoFrames, peek, peek_with};
use support::{Home, fixture, pane_budget, rows};

/// A host whose thumbnail cache holds a 2 x 2 picture of every file but the ones named `none`.
#[derive(Debug)]
struct Cached;

impl VideoFrames for Cached {
    fn frame(&self, source: &anyview_core::Source) -> Option<Rgba8> {
        if source.path().as_path().to_string_lossy().contains("none") {
            return None;
        }
        let size = anyview_core::PixelSize {
            width: anyview_core::PixelLen(2),
            height: anyview_core::PixelLen(2),
        };
        Rgba8::new(size, vec![200; 16]).ok()
    }
}

#[test]
fn a_cached_thumbnail_becomes_the_frame_of_a_video_only() {
    let (video, video_sniffed) = fixture(Home::Media, "clip.mkv");
    let framed = peek_with(&video, &video_sniffed, &pane_budget(), &Cached);
    let Body::Picture(frame) = &framed.body else {
        panic!("expected the frame, got {}", framed.body.slug());
    };
    assert_eq!(frame.picture.size().width.0, 2);
    // The facts are the header's, whatever the picture.
    assert_eq!(
        rows(&framed.facts),
        rows(&peek(&video, &video_sniffed, &pane_budget()).facts)
    );
    // No host source, or none cached: the facts card.
    let bare = peek_with(&video, &video_sniffed, &pane_budget(), &NoFrames);
    assert_eq!(bare.body.slug(), "facts");
    // Audio never takes a video's frame.
    let (audio, audio_sniffed) = fixture(Home::Media, "tone.flac");
    let song = peek_with(&audio, &audio_sniffed, &pane_budget(), &Cached);
    assert_eq!(song.body.slug(), "facts");
}

#[test]
fn a_video_without_a_cached_thumbnail_shows_the_facts_card() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("none.mkv");
    std::fs::copy(support::path(Home::Media, "clip.mkv"), &path).unwrap();
    let (src, sniffed) = support::on_disk(&path, 0);
    let result = peek_with(&src, &sniffed, &pane_budget(), &Cached);
    assert_eq!(result.body.slug(), "facts");
}
