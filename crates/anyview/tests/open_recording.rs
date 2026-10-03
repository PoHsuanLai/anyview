//! A recording opened end to end under the harness with the binary's own wiring and plugins as a
//! person would have them: with none installed it is its facts and the package that would play it,
//! never a blank window, and with the FFmpeg plugin its facts are the plugin's.

#![allow(clippy::unwrap_used)]

mod support;

use ds_harness::Query;
use support::{media_fixture, open, open_with, plugins, until};

/// A copy of the clip in `dir`, which is where the window opens it from.
fn clip(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("clip.mkv");
    std::fs::copy(media_fixture("clip.mkv").as_path(), &file).unwrap();
    file
}

#[test]
fn a_recording_with_no_plugin_is_its_facts_and_the_package_that_would_play_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = clip(dir.path());
    let mut rig = open(&file, dir.path());
    until(&mut rig.harness, "the facts card", |harness| {
        harness.count(".viewer-peek") > 0
    });
    let text = rig.harness.text_of(".viewer").unwrap_or_default();
    assert!(
        text.contains("Needs") && text.contains("anyview-mpv (to play it)"),
        "the row that names the package: {text}"
    );
    assert!(text.contains("clip.mkv"), "the card names the file: {text}");
    assert!(
        text.contains("0:03"),
        "the header's length, read in pure Rust: {text}"
    );
    assert_eq!(rig.harness.count(".viewer-media"), 0, "nothing plays");
    assert!(
        rig.workforce.notices().is_empty(),
        "every job ran to its end and posted its result"
    );
}

#[test]
fn with_the_ffmpeg_plugin_the_facts_are_the_plugins() {
    let Some(plugins) = plugins(false, true) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let file = clip(dir.path());
    let mut rig = open_with(&file, dir.path(), plugins);
    until(&mut rig.harness, "the facts card", |harness| {
        harness.count(".viewer-peek") > 0
    });
    let text = rig.harness.text_of(".viewer").unwrap_or_default();
    assert!(
        text.contains("anyview-mpv"),
        "the package that would play it: {text}"
    );
    assert!(
        text.contains("vorbis") && text.contains("64 × 48"),
        "what ffprobe read: {text}"
    );
}
