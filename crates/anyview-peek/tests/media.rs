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
    let cases: Vec<(&str, Home, bool, Rows)> = vec![
        (
            "clip.mkv",
            Home::Media,
            true,
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
            true,
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
            false,
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
            false,
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
            false,
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
            false,
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
            false,
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
            false,
            vec![
                row("duration", "0:01"),
                row("codec", "pcm"),
                row("sample-rate", "2 kHz"),
                row("channels", "mono"),
                row("bitrate", "32 kbit/s"),
                row("title", "Aiff Tone"),
            ],
        ),
        (
            "song.m4a",
            Home::Own,
            false,
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
            ],
        ),
    ];

    for (name, home, is_video, want) in cases {
        let got = peeked(home, name);
        assert_eq!(header_rows(&got), want, "{name}");
        // A video draws no picture of its own: its body is the facts.
        if is_video {
            assert_eq!(got.kind, FormatKind::Video, "{name}");
            assert_eq!(got.body.slug(), "facts", "{name}");
            assert!(rows(&got.facts)[0].1.starts_with("Video ("), "{name}");
        }
    }
}

/// The cover of `cover.mp3`, at two budgets: a small cover is not enlarged, and one larger than
/// the pixel budget is reduced.
#[test]
fn an_audio_file_with_a_cover_shows_the_cover_reduced_to_the_budget() {
    // step 1: the pane's budget
    let audio = peeked(Home::Media, "cover.mp3");
    assert_eq!(audio.kind, FormatKind::Audio);
    let Body::Picture(cover) = &audio.body else {
        panic!(
            "step pane budget: expected the cover, got {}",
            audio.body.slug()
        );
    };
    assert_eq!(
        (cover.source_size.width.0, cover.source_size.height.0),
        (64, 64)
    );
    assert_eq!(
        cover.picture.size(),
        cover.source_size,
        "step pane budget: a small cover is not enlarged"
    );
    let rows = rows(&audio.facts);
    assert_eq!(rows[0].1, "Audio (MP3)");
    assert_eq!(rows[1], ("duration", "0:02".to_owned()));
    assert_eq!(rows[2], ("codec", "mp3".to_owned()));

    // step 2: a budget of 256 pixels
    let (src, sniffed) = fixture(Home::Media, "cover.mp3");
    let small = support::budget(4_000_000, 256);
    let audio = peek(&src, &sniffed, &small);
    let Body::Picture(cover) = &audio.body else {
        panic!("step small budget: expected the cover");
    };
    assert!(
        cover.picture.size().area().0 <= 256,
        "step small budget: {:?}",
        cover.picture.size()
    );
    assert_eq!(cover.source_size.width.0, 64, "step small budget");
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
    let cases: [(&str, Vec<u8>); 6] = [
        ("cut.mp4", whole[..whole.len() / 2].to_vec()),
        ("cut.mkv", mkv[..40].to_vec()),
        ("cut.flac", flac[..10].to_vec()),
        ("cut.m4a", m4a[..60].to_vec()),
        ("noise.ogg", vec![0; 300]),
        ("broken.mp3", vec![0; 64]),
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

/// Every container that can hold a picture hands it over: ID3 `APIC`, FLAC `PICTURE`, MP4 `covr`
/// and Vorbis `METADATA_BLOCK_PICTURE`; the same containers without one are facts only. The files
/// are made by `fixtures/audio/make.sh`.
#[test]
fn each_audio_container_hands_over_the_cover_it_carries_or_shows_facts_only() {
    // file, whether it carries a cover
    let cases = [
        ("art.mp3", true),
        ("art.flac", true),
        ("art.m4a", true),
        ("art.ogg", true),
        ("plain.mp3", false),
        ("plain.flac", false),
        ("plain.m4a", false),
    ];
    for (name, has_cover) in cases {
        let audio = peeked(Home::Own, &format!("audio/{name}"));
        if has_cover {
            let Body::Picture(cover) = &audio.body else {
                panic!("{name}: expected the cover, got {}", audio.body.slug());
            };
            assert_eq!(
                (cover.source_size.width.0, cover.source_size.height.0),
                (64, 64),
                "{name}"
            );
        } else {
            assert_eq!(audio.body.slug(), "facts", "{name}");
        }
    }
}

/// A cover comes from the bytes handed in, with no file behind them.
#[test]
fn a_cover_is_read_from_bytes_that_are_no_file() {
    let path = support::path(Home::Own, "audio/art.flac");
    let bytes = std::fs::read(&path).unwrap();
    let input = anyview_core::Input::from((
        anyview_core::FileName::new("art.flac").unwrap(),
        bytes.clone(),
    ));
    let sniffed = support::sniffed(&bytes, "art.flac");
    let cover = anyview_peek::audio_cover(&input, &sniffed, &pane_budget()).unwrap();
    assert_eq!((cover.size.width.0, cover.size.height.0), (64, 64));
    assert!(anyview_peek::is_audio(&input));
}
