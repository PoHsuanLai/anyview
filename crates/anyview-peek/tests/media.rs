//! Peeking at recordings through the registry: the facts and tags each header gives, the cover an
//! audio file carries, and the frame a host may add to a video.

#![cfg(feature = "media")]
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FormatKind;
use anyview_peek::{Body, peek};
use support::{Home, fixture, pane_budget, rows};

fn peeked(home: Home, name: &str) -> anyview_peek::AnyPeeked {
    let (src, sniffed) = fixture(home, name);
    peek(&src, &sniffed, &pane_budget())
}

/// The rows a header gave, leaving out the two every file has (size and date) and the kind row.
fn header_rows(peeked: &anyview_peek::AnyPeeked) -> Vec<(&'static str, String)> {
    rows(&peeked.facts)
        .into_iter()
        .filter(|(slug, _)| !matches!(*slug, "kind" | "size" | "modified"))
        .collect()
}

/// The rows of a header, as `(label slug, text)`.
type Rows = Vec<(&'static str, String)>;

fn row(slug: &'static str, text: &str) -> (&'static str, String) {
    (slug, text.to_owned())
}

/// Every format read, with the rows its header gave. These are the values `ffprobe` reports for
/// the same files (the fixtures were made with ffmpeg; see the commit that added them), except that
/// the bitrate is the file's size over its length.
#[test]
fn each_format_lists_what_its_header_says() {
    let cases: Vec<(&str, Home, Rows)> = vec![
        (
            "clip.mkv",
            Home::Media,
            vec![
                row("duration", "0:03"),
                row("dimensions", "64 × 48"),
                row("codec", "mpeg4"),
                row("audio-codec", "vorbis"),
                row("sample-rate", "8 kHz"),
                row("channels", "mono"),
                row("bitrate", "40 kbit/s"),
                row("streams", "1 video, 2 audio, 1 subtitles"),
            ],
        ),
        (
            "clip.mp4",
            Home::Own,
            vec![
                row("duration", "0:01"),
                row("dimensions", "64 × 48"),
                row("codec", "mpeg4"),
                row("audio-codec", "aac"),
                row("sample-rate", "8 kHz"),
                row("channels", "mono"),
                row("bitrate", "45 kbit/s"),
                row("streams", "1 video, 1 audio"),
                row("title", "Test Clip"),
                row("author", "Quire"),
                row("album", "Fixtures"),
            ],
        ),
        (
            "tone.flac",
            Home::Media,
            vec![
                row("duration", "0:02"),
                row("codec", "flac"),
                row("sample-rate", "8 kHz"),
                row("channels", "mono"),
                row("bitrate", "63 kbit/s"),
            ],
        ),
        (
            "tagged.mp3",
            Home::Own,
            vec![
                row("duration", "0:01"),
                row("codec", "mp3"),
                row("sample-rate", "8 kHz"),
                row("channels", "mono"),
                row("bitrate", "40 kbit/s"),
                row("title", "MP3 Tone"),
                row("author", "Quire"),
                row("album", "Fixtures"),
                row("track-number", "5"),
            ],
        ),
        (
            "tone.ogg",
            Home::Own,
            vec![
                row("duration", "0:01"),
                row("codec", "vorbis"),
                row("sample-rate", "8 kHz"),
                row("channels", "mono"),
                row("bitrate", "32 kbit/s"),
                row("title", "Vorbis Tone"),
                row("author", "Quire"),
                row("album", "Fixtures"),
                row("track-number", "1"),
            ],
        ),
        (
            "tone.opus",
            Home::Own,
            vec![
                row("duration", "0:01"),
                row("codec", "opus"),
                row("sample-rate", "48 kHz"),
                row("channels", "mono"),
                row("bitrate", "32 kbit/s"),
                row("title", "Opus Tone"),
                row("author", "Quire"),
                row("album", "Fixtures"),
                row("track-number", "3"),
            ],
        ),
        (
            "tone.wav",
            Home::Own,
            vec![
                row("duration", "0:01"),
                row("codec", "pcm"),
                row("sample-rate", "2 kHz"),
                row("channels", "mono"),
                row("bitrate", "32 kbit/s"),
            ],
        ),
        (
            "tone.aiff",
            Home::Own,
            vec![
                row("duration", "0:01"),
                row("codec", "pcm"),
                row("sample-rate", "2 kHz"),
                row("channels", "mono"),
                row("bitrate", "32 kbit/s"),
                row("title", "Aiff Tone"),
            ],
        ),
    ];
    for (name, home, want) in cases {
        assert_eq!(header_rows(&peeked(home, name)), want, "{name}");
    }
}

#[test]
fn a_video_draws_no_picture_of_its_own() {
    for name in ["clip.mkv", "clip.mp4"] {
        let home = if name == "clip.mkv" {
            Home::Media
        } else {
            Home::Own
        };
        let video = peeked(home, name);
        assert_eq!(video.kind, FormatKind::Video, "{name}");
        assert_eq!(video.body.slug(), "facts", "{name}");
        assert!(rows(&video.facts)[0].1.starts_with("Video ("), "{name}");
    }
}

#[test]
fn an_audio_file_with_a_cover_shows_the_cover_reduced_to_the_budget() {
    let audio = peeked(Home::Media, "cover.mp3");
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
fn an_m4a_shows_its_tags_and_its_cover() {
    let audio = peeked(Home::Own, "song.m4a");
    let Body::Picture(cover) = &audio.body else {
        panic!("expected the cover, got {}", audio.body.slug());
    };
    assert_eq!(
        (cover.source_size.width.0, cover.source_size.height.0),
        (64, 64)
    );
    assert_eq!(
        header_rows(&audio),
        vec![
            row("duration", "0:01"),
            row("codec", "aac"),
            row("sample-rate", "8 kHz"),
            row("channels", "mono"),
            row("bitrate", "32 kbit/s"),
            row("title", "M4A Tone"),
            row("author", "Quire"),
            row("album", "Fixtures"),
            row("track-number", "2"),
        ]
    );
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
    let flac = peeked(Home::Media, "tone.flac");
    assert_eq!(flac.body.slug(), "facts");
    assert_eq!(rows(&flac.facts)[1], ("duration", "0:02".to_owned()));
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.mp3");
    std::fs::write(&broken, vec![0_u8; 64]).unwrap();
    let (src, sniffed) = support::on_disk(&broken, 0);
    let result = peek(&src, &sniffed, &pane_budget());
    assert_eq!(result.body.slug(), "unavailable");
}

/// The recordings no parser here reads show what any file shows: its type, size and date.
#[test]
fn a_format_nothing_parses_gets_facts_only() {
    let dir = tempfile::tempdir().unwrap();
    let cases: [(&str, &[u8]); 2] = [
        ("movie.avi", b"RIFF\x24\0\0\0AVI LIST\0\0\0\0hdrl"),
        ("movie.flv", b"FLV\x01\x05\0\0\0\x09\0\0\0\0"),
    ];
    for (name, bytes) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let (src, sniffed) = support::on_disk(&path, 0);
        let result = peek(&src, &sniffed, &pane_budget());
        assert_eq!(result.kind, FormatKind::Video, "{name}");
        assert_eq!(result.body.slug(), "facts", "{name}");
        let slugs: Vec<&str> = rows(&result.facts).iter().map(|(slug, _)| *slug).collect();
        assert_eq!(slugs, vec!["kind", "size", "modified"], "{name}");
    }
}

#[test]
fn a_damaged_recording_says_why_whichever_parser_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let whole = std::fs::read(support::path(Home::Own, "clip.mp4")).unwrap();
    let mkv = std::fs::read(support::path(Home::Media, "clip.mkv")).unwrap();
    let flac = std::fs::read(support::path(Home::Media, "tone.flac")).unwrap();
    let m4a = std::fs::read(support::path(Home::Own, "song.m4a")).unwrap();
    let cases: [(&str, Vec<u8>); 5] = [
        ("cut.mp4", whole[..whole.len() / 2].to_vec()),
        ("cut.mkv", mkv[..40].to_vec()),
        ("cut.flac", flac[..10].to_vec()),
        ("cut.m4a", m4a[..60].to_vec()),
        ("noise.ogg", vec![0; 300]),
    ];
    for (name, bytes) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let (src, sniffed) = support::on_disk(&path, 0);
        let result = peek(&src, &sniffed, &pane_budget());
        assert_eq!(result.body.slug(), "unavailable", "{name}");
    }
}

/// A header whose elements loop sends the demuxer round forever; the peek must give up and say the
/// file is unavailable instead of holding its worker.
#[test]
fn a_matroska_header_that_loops_is_given_up_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut mkv = std::fs::read(support::path(Home::Media, "clip.mkv")).unwrap();
    mkv[51..58].fill(0);
    let path = dir.path().join("loop.mkv");
    std::fs::write(&path, mkv).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (src, sniffed) = support::on_disk(&path, 0);
        let result = peek(&src, &sniffed, &pane_budget());
        let _ = sender.send(result.body.slug());
    });
    let slug = receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the peek ends");
    assert_eq!(slug, "unavailable");
}
