//! A window's session on an audio file, played by the built-in player (sound off, `AudioDriver::Null`):
//! the desktop's entry carries the title, the artist and the cover, it can skip, a media key's Next
//! and Previous reach the window as news, and the player ends with its window. Skipped, saying so,
//! on a machine with no graphics adapter (the window's texture needs one).

use crate::support;

use anyview::media::{MediaHub, PlayerHost};
use anyview_core::FilePath;
use anyview_media::AudioDriver;
use anyview_platform::testing::{FakeMediaHandle, FakeMediaSession};
use anyview_platform::{Ability, MediaControl, PlaybackStatus};
use anyview_ui::{
    MediaHost, MediaLine, MediaNotice, MediaPlayback, MediaStart, MediaWake, PlayerEvent,
};
use std::sync::Arc;
use support::{eventually, fixture, gpu, plugins, probed};
use tokio::runtime::Runtime;

fn audio(name: &str) -> FilePath {
    FilePath::new(std::fs::canonicalize(fixture("anyview-peek", &format!("audio/{name}"))).unwrap())
        .unwrap()
}

struct Rig {
    _runtime: Runtime,
    hub: MediaHub,
    desk: FakeMediaHandle,
}

fn rig() -> Option<Rig> {
    let plugins = plugins(false, false)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let session = FakeMediaSession::new();
    let desk = session.handle();
    let hub = MediaHub::start(
        runtime.handle(),
        move || std::future::ready(session),
        None,
        AudioDriver::Null,
        plugins,
    );
    Some(Rig {
        _runtime: runtime,
        hub,
        desk,
    })
}

fn open(
    host: &PlayerHost,
    gpu: &ds_blitz::Gpu,
    file: &FilePath,
) -> (Arc<dyn MediaLine>, anyview_ui::MediaStarted) {
    let started = host
        .start(MediaStart {
            file: file.clone(),
            source: probed(file).source,
            sniffed: probed(file).sniffed,
            texture: gpu.handle(),
            wake: MediaWake::new(|| {}),
        })
        .unwrap();
    let MediaPlayback::Line(line) = &started.playback else {
        panic!("the built-in player was started");
    };
    (Arc::clone(line), started)
}

#[test]
fn an_audio_window_tells_the_desktop_what_plays_and_obeys_its_media_keys() {
    let Some(gpu) = gpu() else {
        return;
    };
    let Some(rig) = rig() else {
        return;
    };
    let host = PlayerHost::new(rig.hub.clone());
    let file = audio("art.mp3");
    let (line, started) = open(&host, &gpu, &file);
    assert!(started.cover.is_some(), "the cover the file carries");

    let mut heard = Vec::new();
    eventually("the file to load", || {
        heard.extend(line.drain());
        heard
            .iter()
            .any(|n| matches!(n, MediaNotice::Player(PlayerEvent::Loaded { .. })))
    });
    eventually("the entry to say playing", || {
        rig.desk
            .published()
            .last()
            .is_some_and(|state| state.status == PlaybackStatus::Playing)
    });
    let entry = rig.desk.published().pop().unwrap();
    assert_eq!(entry.title.as_deref(), Some("SineSong"));
    assert_eq!(entry.artist.as_deref(), Some("TheTones"));
    assert_eq!(entry.album.as_deref(), Some("TestTones"));
    assert!(entry.art.is_some(), "the entry carries the cover");
    assert!(entry.length.is_some());
    assert_eq!(entry.skip, Ability::Can, "a window walks its folder");

    rig.desk.press(MediaControl::Next);
    rig.desk.press(MediaControl::Previous);
    eventually("the keys to reach the window", || {
        heard.extend(line.drain());
        heard.contains(&MediaNotice::Next) && heard.contains(&MediaNotice::Previous)
    });

    rig.desk.press(MediaControl::Pause);
    eventually("the entry to say paused", || {
        rig.desk
            .published()
            .last()
            .is_some_and(|state| state.status == PlaybackStatus::Paused)
    });

    // The window lets its player go: the entry empties.
    drop(line);
    drop(started);
    eventually("the entry to stop", || {
        rig.desk
            .published()
            .last()
            .is_some_and(|state| state.status == PlaybackStatus::Stopped && state.title.is_none())
    });
}

#[test]
fn a_file_with_no_cover_has_none_to_show_or_tell() {
    let Some(gpu) = gpu() else {
        return;
    };
    let Some(rig) = rig() else {
        return;
    };
    let host = PlayerHost::new(rig.hub.clone());
    let (_line, started) = open(&host, &gpu, &audio("plain.flac"));
    assert!(started.cover.is_none());
}
