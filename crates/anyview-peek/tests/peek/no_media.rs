//! The launcher built without libav (`--no-default-features`): a recording is a file whose kind has
//! no back end, so it shows its type, size and date and never panics.
#![cfg(not(feature = "media"))]

use crate::support;

use anyview_core::FormatKind;
use anyview_peek::peek;
use support::{Home, fixture, pane_budget, rows};

#[test]
fn a_recording_shows_its_type_size_and_date_and_nothing_libav_would_give() {
    for (name, kind, label) in [
        ("clip.mkv", FormatKind::Video, "Video ("),
        ("cover.mp3", FormatKind::Audio, "Audio (MP3)"),
        ("tone.flac", FormatKind::Audio, "Audio ("),
    ] {
        let (src, sniffed) = fixture(Home::Media, name);
        let peeked = peek(&src, &sniffed, &pane_budget());
        assert_eq!(peeked.kind, kind, "{name}");
        assert_eq!(peeked.body.slug(), "facts", "{name}");
        let rows = rows(&peeked.facts);
        let slugs: Vec<&str> = rows.iter().map(|(slug, _)| *slug).collect();
        assert_eq!(slugs, vec!["kind", "size", "modified"], "{name}");
        assert!(rows[0].1.starts_with(label), "{name}: {}", rows[0].1);
    }
}
