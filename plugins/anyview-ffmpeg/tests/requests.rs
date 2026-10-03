//! Probe, thumbnail and decode through the host's `PluginRunner`, against the plugin built for
//! this run and the fixtures of the media crate.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{PixelArea, PixelLen};
use anyview_platform::{PlatformError, PluginRunner};
use anyview_plugin_protocol::{ErrorCode, FactRow};
use support::{Scratch, file, fixture};

fn rows(rows: &[FactRow]) -> Vec<(&str, &str)> {
    rows.iter()
        .map(|row| (row.label.as_str(), row.value.as_str()))
        .collect()
}

fn probe(name: &str) -> Vec<FactRow> {
    let scratch = Scratch::new();
    let plugin = scratch.install(&[]);
    PluginRunner::default()
        .probe(&plugin, &file(&fixture(name)))
        .unwrap()
}

#[test]
fn a_video_with_sound_chapters_and_subtitles_has_its_facts() {
    require_ffmpeg!();
    let got = probe("clip.mkv");
    let got = rows(&got);
    let want = [
        ("duration", "0:03"),
        ("dimensions", "64 × 48"),
        ("codec", "mpeg4"),
        ("framerate", "10 fps"),
        ("audio-codec", "vorbis"),
        ("sample-rate", "8 kHz"),
        ("channels", "mono"),
    ];
    assert_eq!(got[..want.len()], want, "{got:?}");
    assert!(
        got.contains(&("streams", "1 video, 2 audio, 1 subtitle")),
        "{got:?}"
    );
    assert!(got.contains(&("chapters", "2 chapters")), "{got:?}");
}

#[test]
fn every_row_the_plugin_sends_is_a_label_the_viewer_knows() {
    require_ffmpeg!();
    use anyview_core::FactLabel;
    use ds_core::word::Word;
    for name in ["clip.mkv", "tone.flac", "cover.mp3"] {
        for row in probe(name) {
            assert!(
                FactLabel::parse(&row.label).is_some(),
                "{name}: the viewer drops the row {:?}, which it does not know",
                row.label
            );
        }
    }
}

#[test]
fn a_sound_has_a_plain_codec_and_a_cover_is_counted() {
    require_ffmpeg!();
    let flac = probe("tone.flac");
    assert_eq!(
        rows(&flac)[..4],
        [
            ("duration", "0:02"),
            ("codec", "flac"),
            ("sample-rate", "8 kHz"),
            ("channels", "mono"),
        ]
    );
    let mp3 = probe("cover.mp3");
    let mp3 = rows(&mp3);
    assert!(mp3.contains(&("codec", "mp3")), "{mp3:?}");
    assert!(mp3.contains(&("bitrate", "16 kbit/s")), "{mp3:?}");
    assert!(mp3.contains(&("streams", "1 audio, 1 cover")), "{mp3:?}");
}

#[test]
fn a_file_that_is_not_a_recording_is_a_corrupt_error_and_a_missing_one_unreadable() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let plugin = scratch.install(&[]);
    let runner = PluginRunner::default();
    let junk = scratch.path("junk.mkv");
    std::fs::write(&junk, b"this is not a recording at all, just some text").unwrap();
    let error = runner.probe(&plugin, &file(&junk)).unwrap_err();
    assert!(
        matches!(
            &error,
            PlatformError::PluginFailed {
                code: ErrorCode::Corrupt,
                ..
            }
        ),
        "{error:?}"
    );
    let error = runner
        .probe(&plugin, &file(&scratch.path("absent.mkv")))
        .unwrap_err();
    assert!(
        matches!(
            &error,
            PlatformError::PluginFailed {
                code: ErrorCode::Unreadable,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_thumbnail_of_a_video_is_a_frame_inside_the_edge() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let plugin = scratch.install(&[]);
    let runner = PluginRunner::default();
    let clip = file(&fixture("clip.mkv"));
    let small = runner.thumbnail(&plugin, &clip, PixelLen(32)).unwrap();
    assert_eq!((small.size().width.0, small.size().height.0), (32, 24));
    assert_eq!(small.rgba().len(), 32 * 24 * 4);
    assert!(small.rgba().chunks(4).all(|px| px[3] == 255), "opaque");
    assert!(
        small.rgba().iter().any(|byte| *byte != 0),
        "not an empty frame"
    );
    // A video smaller than the edge is not enlarged.
    let whole = runner.thumbnail(&plugin, &clip, PixelLen(256)).unwrap();
    assert_eq!((whole.size().width.0, whole.size().height.0), (64, 48));
}

#[test]
fn the_cover_of_a_sound_is_its_thumbnail_and_a_bare_sound_has_none() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let plugin = scratch.install(&[]);
    let runner = PluginRunner::default();
    let cover = runner
        .thumbnail(&plugin, &file(&fixture("cover.mp3")), PixelLen(32))
        .unwrap();
    assert_eq!((cover.size().width.0, cover.size().height.0), (32, 32));
    assert!(cover.rgba().iter().any(|byte| *byte != 0));
    let error = runner
        .thumbnail(&plugin, &file(&fixture("tone.flac")), PixelLen(32))
        .unwrap_err();
    assert!(
        matches!(
            &error,
            PlatformError::PluginFailed {
                code: ErrorCode::Unsupported,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_decode_is_scaled_to_the_pixel_budget_and_never_enlarged() {
    require_ffmpeg!();
    let scratch = Scratch::new();
    let plugin = scratch.install(&[]);
    let runner = PluginRunner::default();
    let clip = file(&fixture("clip.mkv"));
    let small = runner.decode(&plugin, &clip, PixelArea(1_000)).unwrap();
    assert!(
        small.size().area() <= PixelArea(1_000),
        "{:?}",
        small.size()
    );
    assert_eq!(small.rgba().len() as u64, small.size().area().0 * 4);
    let whole = runner
        .decode(&plugin, &clip, PixelArea(10_000_000))
        .unwrap();
    assert_eq!((whole.size().width.0, whole.size().height.0), (64, 48));
}
