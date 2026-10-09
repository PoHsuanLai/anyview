//! A recording opened end to end under the harness with the binary's own wiring and plugins as a
//! person would have them: with none installed a video is its facts and the package that would
//! play it, never a blank window, and an audio file plays in the built-in player (Opus, which it
//! has no decoder for, is the facts and the package); with the FFmpeg plugin its facts are the
//! plugin's.

use crate::support;

use anyview_platform::{MediaControl, MediaState, PlaybackStatus};
use ds::prelude::{Point, Px};
use ds_harness::{Driver, Input, Query};
use support::{copy_into, media_fixture, open, open_with, plugins, until};

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

/// The window drawn with the pointer on it, so the capsule shows.
fn hover(harness: &mut ds_harness::Harness) {
    harness.send(Input::pointer_move(Point {
        x: Px(450.0),
        y: Px(300.0),
    }));
}

/// The entry the desktop was last given.
fn entry(now_playing: &anyview_platform::testing::FakeMediaHandle) -> Option<MediaState> {
    now_playing.published().last().cloned()
}

#[cfg(feature = "audio")]
#[test]
fn an_audio_file_with_no_mpv_plays_in_the_built_in_player_and_the_desktops_keys_control_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = copy_into(dir.path(), "sine.mp3");
    let mut rig = open(file.as_path(), dir.path());
    until(&mut rig.harness, "the playing capsule", |harness| {
        hover(harness);
        harness.count(".ds-scrubber") == 1
            && harness
                .text_of(".ds-capsule")
                .is_some_and(|text| text.contains("0:0"))
    });
    assert_eq!(rig.harness.count(".viewer-peek"), 0, "not the facts card");
    let text = rig.harness.text_of(".ds-capsule").unwrap_or_default();
    assert!(text.contains("0:02"), "the length: {text}");
    assert_eq!(rig.harness.count(".ds-capsule .ds-slider"), 1, "the volume");
    assert!(
        !text.contains('×') && !rig.harness.html().contains("Faster"),
        "the built-in player has no speed control: {text}"
    );
    assert_eq!(
        rig.harness.count(".viewer-media-status"),
        0,
        "nothing left to say"
    );

    until(&mut rig.harness, "the desktop's entry to play", |_| {
        entry(&rig.now_playing).is_some_and(|state| state.status == PlaybackStatus::Playing)
    });
    let state = entry(&rig.now_playing).unwrap();
    assert_eq!(
        state.title.as_deref(),
        Some("sine.mp3"),
        "titled by its file"
    );
    until(&mut rig.harness, "the position to move", |_| {
        entry(&rig.now_playing).is_some_and(|state| state.position.0 >= 300_000)
    });

    rig.now_playing.press(MediaControl::Pause);
    until(
        &mut rig.harness,
        "the pause to reach the desktop's entry",
        |_| entry(&rig.now_playing).is_some_and(|state| state.status == PlaybackStatus::Paused),
    );
    let held = entry(&rig.now_playing).unwrap().position;
    rig.now_playing
        .press(MediaControl::SeekTo(anyview_core::MediaTime::from_millis(
            100,
        )));
    until(&mut rig.harness, "the seek to land", |_| {
        entry(&rig.now_playing).is_some_and(|state| state.position < held)
    });
    rig.now_playing.press(MediaControl::Play);
    until(
        &mut rig.harness,
        "the play to reach the desktop's entry",
        |_| entry(&rig.now_playing).is_some_and(|state| state.status == PlaybackStatus::Playing),
    );
}

#[cfg(feature = "audio")]
#[test]
fn an_audio_file_plays_to_its_end_and_stops_there() {
    let dir = tempfile::tempdir().unwrap();
    let file = copy_into(dir.path(), "ramp.flac");
    let mut rig = open(file.as_path(), dir.path());
    until(&mut rig.harness, "the end of the recording", |harness| {
        hover(harness);
        entry(&rig.now_playing).is_some_and(|state| state.status == PlaybackStatus::Stopped)
    });
    let state = entry(&rig.now_playing).unwrap();
    assert_eq!(
        state.position,
        state.length.unwrap().0,
        "stopped at the end"
    );
}

#[test]
fn an_opus_file_with_no_mpv_is_its_facts_and_the_package_that_would_play_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = copy_into(dir.path(), "sine.opus");
    let mut rig = open(file.as_path(), dir.path());
    until(&mut rig.harness, "the facts card", |harness| {
        harness.count(".viewer-peek") > 0
    });
    let text = rig.harness.text_of(".viewer").unwrap_or_default();
    assert!(
        text.contains("Needs") && text.contains("anyview-mpv (to play it)"),
        "the row that names the package: {text}"
    );
    assert_eq!(rig.harness.count(".ds-scrubber"), 0, "nothing plays");
}
