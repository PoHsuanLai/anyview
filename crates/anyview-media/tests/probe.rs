//! What libav reads of the three fixtures, and the rows a pane lists from it.

#![allow(clippy::unwrap_used)]

mod libav;

use anyview_core::{
    FactLabel, MediaContainer, MediaTags, MediaTrack, Peek, StreamKind, TrackId, TrackPlay,
};
use anyview_media::{AudioPeek, CoverCodec, MediaPeeked, VideoPeek};
use libav::{fixture, probed, seconds};

fn track(
    id: u32,
    kind: StreamKind,
    title: Option<&str>,
    language: Option<&str>,
    codec: &str,
) -> MediaTrack {
    MediaTrack {
        id: TrackId(id),
        kind,
        title: title.map(str::to_owned),
        language: language.map(str::to_owned),
        codec: Some(codec.to_owned()),
        play: TrackPlay::Idle,
    }
}

#[test]
fn a_matroska_clip_has_its_streams_numbered_as_the_player_numbers_them() {
    let probe = probed(&fixture("clip.mkv"));
    assert!(
        (seconds(&probe) - 3.032).abs() < 0.01,
        "{}",
        seconds(&probe)
    );
    assert_eq!(probe.container, Some(MediaContainer::Mkv));
    assert_eq!(
        probe.tracks,
        vec![
            track(1, StreamKind::Video, None, None, "mpeg4"),
            track(1, StreamKind::Audio, Some("Main"), Some("eng"), "vorbis"),
            track(2, StreamKind::Audio, None, Some("deu"), "vorbis"),
            track(1, StreamKind::Subtitles, None, Some("eng"), "subrip"),
        ]
    );
    let video = probe.video.as_ref().unwrap();
    assert_eq!(
        (
            video.codec.as_str(),
            video.size.width.0,
            video.size.height.0
        ),
        ("mpeg4", 64, 48)
    );
    let audio = probe.audio.as_ref().unwrap();
    assert_eq!(
        (audio.codec.as_str(), audio.channels, audio.sample_rate),
        ("vorbis", 1, 8000)
    );
    let chapters: Vec<(&str, u64)> = probe
        .chapters
        .iter()
        .map(|chapter| (chapter.title.as_str(), chapter.start.0))
        .collect();
    assert_eq!(chapters, vec![("Opening", 0), ("Middle", 1_000_000)]);
    assert!(probe.cover.is_none());
}

#[test]
fn an_mp3_with_a_cover_keeps_the_picture_encoded_and_shows_no_video() {
    let probe = probed(&fixture("cover.mp3"));
    assert!((seconds(&probe) - 2.0).abs() < 0.1, "{}", seconds(&probe));
    assert_eq!(probe.container, Some(MediaContainer::Mp3));
    assert!(probe.video.is_none(), "a cover is not a video");
    let cover = probe.cover.as_ref().expect("the cover");
    assert_eq!(cover.codec, CoverCodec::Png);
    assert_eq!(&cover.bytes[..8], b"\x89PNG\r\n\x1a\n");
    let kinds: Vec<StreamKind> = probe.tracks.iter().map(|track| track.kind).collect();
    assert_eq!(kinds, vec![StreamKind::Audio, StreamKind::Video]);
}

#[test]
fn a_cover_over_the_limit_is_left_out() {
    let small = anyview_media::probe_within(&fixture("cover.mp3"), 8).unwrap();
    assert!(small.cover.is_none());
}

#[test]
fn a_flac_is_audio_with_no_picture() {
    let probe = probed(&fixture("tone.flac"));
    assert_eq!(probe.container, Some(MediaContainer::Flac));
    assert!(probe.video.is_none() && probe.cover.is_none());
    assert_eq!(probe.audio.as_ref().unwrap().codec, "flac");
    assert_eq!(probe.tags, MediaTags::default());
}

#[test]
fn a_file_that_is_not_media_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.mkv");
    std::fs::write(&path, b"plainly not a recording").unwrap();
    assert!(anyview_media::probe(&path).is_err());
    assert!(anyview_media::probe(&dir.path().join("missing.mkv")).is_err());
}

#[test]
fn the_pane_lists_what_the_probe_read() {
    let rows = |peeked: &MediaPeeked,
                facts: fn(&MediaPeeked) -> anyview_core::Facts|
     -> Vec<(FactLabel, String)> {
        facts(peeked)
            .rows()
            .iter()
            .map(|row| (row.label, row.value.as_str().to_owned()))
            .collect()
    };
    let video = MediaPeeked {
        probe: probed(&fixture("clip.mkv")),
    };
    let flac = MediaPeeked {
        probe: probed(&fixture("tone.flac")),
    };
    let cover = MediaPeeked {
        probe: probed(&fixture("cover.mp3")),
    };
    let video_rows = rows(&video, VideoPeek::facts);
    assert_eq!(video_rows[0], (FactLabel::Duration, "0:03".to_owned()));
    assert_eq!(video_rows[1], (FactLabel::Dimensions, "64 × 48".to_owned()));
    assert_eq!(video_rows[2], (FactLabel::Codec, "mpeg4".to_owned()));
    assert_eq!(video_rows[3], (FactLabel::AudioCodec, "vorbis".to_owned()));
    let flac_rows = rows(&flac, AudioPeek::facts);
    assert_eq!(flac_rows[0], (FactLabel::Duration, "0:02".to_owned()));
    assert_eq!(flac_rows[1], (FactLabel::Codec, "flac".to_owned()));
    let cover_rows = rows(&cover, AudioPeek::facts);
    assert_eq!(cover_rows[0], (FactLabel::Duration, "0:02".to_owned()));
    assert_eq!(cover_rows[1], (FactLabel::Codec, "mp3".to_owned()));
}
