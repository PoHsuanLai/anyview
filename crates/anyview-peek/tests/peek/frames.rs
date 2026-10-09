//! The host's source of a video's frame: a cached thumbnail stands in for the picture a launcher
//! cannot decode, and only for a video that shows nothing else.

#![cfg(feature = "media")]

use crate::support;

use anyview_image::Rgba8;
use anyview_peek::{Body, NoStills, Peeking, StillSource, look, peek};
use std::sync::Arc;
use support::{Home, fixture, pane_budget, pane_looking, rows};

/// A host whose thumbnail cache holds a 2 x 2 picture of every file but the ones named `none`.
#[derive(Debug)]
struct Cached;

impl StillSource for Cached {
    fn still(&self, source: &anyview_core::Input) -> Option<Rgba8> {
        if source.name().as_str().contains("none") {
            return None;
        }
        let size = anyview_core::PixelSize {
            width: anyview_core::PixelLen(2),
            height: anyview_core::PixelLen(2),
        };
        Rgba8::new(size, vec![200; 16]).ok()
    }
}

fn with(stills: Arc<dyn StillSource>) -> Peeking {
    pane_looking().with_stills(stills)
}

#[test]
fn a_cached_thumbnail_becomes_the_frame_of_a_video_only() {
    let (video, video_sniffed) = fixture(Home::Media, "clip.mkv");
    let framed = look(video.clone(), &with(Arc::new(Cached)));
    let Body::Picture(frame) = &framed.body else {
        panic!("expected the frame, got {}", framed.body.slug());
    };
    assert_eq!(frame.picture.size().width.0, 2);
    assert_eq!(framed.still().map(|still| still.size().width.0), Some(2));
    // The facts are the header's, whatever the picture.
    assert_eq!(
        rows(&framed.facts),
        rows(&peek(&video, &video_sniffed, &pane_budget()).facts)
    );
    // No host source, or none cached: the facts card.
    let bare = look(video, &with(Arc::new(NoStills)));
    assert_eq!(bare.body.slug(), "facts");
    assert!(bare.still().is_none());
    // A video the host has no cached thumbnail for (the stand-in refuses names with `none`) also
    // shows the facts card.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("none.mkv");
    std::fs::copy(support::path(Home::Media, "clip.mkv"), &path).unwrap();
    let (uncached, _) = support::on_disk(&path, 0);
    let result = look(uncached, &with(Arc::new(Cached)));
    assert_eq!(
        result.body.slug(),
        "facts",
        "step none.mkv: no cached thumbnail"
    );
    // Audio never takes a video's frame.
    let (audio, _) = fixture(Home::Media, "tone.flac");
    let song = look(audio, &with(Arc::new(Cached)));
    assert_eq!(song.body.slug(), "facts");
}
