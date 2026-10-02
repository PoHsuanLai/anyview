//! The program's players with the real player: a window's session plays a fixture on its own
//! thread, its picture reaches the window's texture, the desktop's now-playing entry follows it
//! and the desktop's controls obey; a session with no window plays, is stopped from the entry and
//! ends when its recording does; and the media exports a window asks for are written beside the
//! file. Sound is off (`ao=null`). A machine with no graphics adapter skips them, saying so.

#![allow(clippy::unwrap_used)]

mod support;

use anyview::host::{
    Clock, Desktop, Hosting, Media, Outcome, Services, Store, Task, Trash, TrashError,
};
use anyview::media::{MediaHub, PlayerHost};
use anyview::runtime::PoolSize;
use anyview::seam::{NoticeWaker, Workforce};
use anyview_core::{
    ByteLen, FileHead, FileName, FilePath, FileStamp, MediaExport, MediaTime, ModTime, Percent,
    RasterTarget, Resume, SniffStep, Source, TimeRange, Volume, sniff,
};
use anyview_media::AudioDriver;
use anyview_platform::testing::{
    FakeApps, FakeMediaHandle, FakeMediaSession, FakePrinter, FakeReveal, FakeShare,
};
use anyview_platform::{MediaControl, PlaybackStatus, PrintOutcome};
use anyview_store::Viewed;
use anyview_ui::{
    MediaHost, MediaLine, MediaNotice, MediaStart, MediaWake, Pace, PlayerEvent, Probed,
    SlotPixels, StageFamily, family_of,
};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::mpsc::channel;
use support::{eventually, gpu, media_fixture};
use tokio::runtime::Runtime;

struct NoTrash;

impl Trash for NoTrash {
    fn trash(&self, _file: &FilePath) -> Result<(), TrashError> {
        Ok(())
    }
}

struct Rig {
    runtime: Runtime,
    hub: MediaHub,
    desk: FakeMediaHandle,
    workforce: Arc<Workforce>,
}

fn rig() -> Rig {
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
    );
    let workforce = Arc::new(
        Workforce::start(
            PoolSize::exactly(std::num::NonZeroUsize::new(2).unwrap()),
            NoticeWaker::default(),
        )
        .unwrap(),
    );
    Rig {
        runtime,
        hub,
        desk,
        workforce,
    }
}

fn last_state(desk: &FakeMediaHandle) -> Option<anyview_platform::MediaState> {
    desk.published().pop()
}

/// A window's player on `file`, its picture 64 by 48, and everything it has said so far.
struct Window {
    line: Arc<dyn MediaLine>,
    heard: Vec<MediaNotice>,
    wakes: std::sync::mpsc::Receiver<()>,
    texture: ds_blitz::TextureHandle,
    started: anyview_ui::MediaStarted,
}

impl Window {
    fn open(host: &PlayerHost, gpu: &ds_blitz::Gpu, file: &FilePath) -> Window {
        let (tx, wakes) = channel();
        let texture = gpu.handle();
        let started = host
            .start(MediaStart {
                file: file.clone(),
                texture: texture.clone(),
                wake: MediaWake::new(move || {
                    let _ = tx.send(());
                }),
            })
            .unwrap();
        let line = Arc::clone(&started.line);
        line.resize(Some(SlotPixels {
            width: NonZeroU32::new(64).unwrap(),
            height: NonZeroU32::new(48).unwrap(),
        }));
        Window {
            line,
            heard: Vec::new(),
            wakes,
            texture,
            started,
        }
    }

    fn listen(&mut self, what: &str, done: impl Fn(&[MediaNotice]) -> bool) {
        eventually(what, || {
            let _ = self
                .wakes
                .recv_timeout(std::time::Duration::from_millis(20));
            self.heard.extend(self.line.drain());
            done(&self.heard)
        });
    }
}

fn is_loaded(notices: &[MediaNotice]) -> bool {
    notices
        .iter()
        .any(|notice| matches!(notice, MediaNotice::Player(PlayerEvent::Loaded { .. })))
}

#[test]
fn a_window_session_plays_shows_its_picture_and_obeys_the_desktop() {
    let Some(gpu) = gpu() else {
        return;
    };
    let rig = rig();
    let host = PlayerHost::new(rig.hub.clone());
    let clip = media_fixture("clip.mkv");
    let mut window = Window::open(&host, &gpu, &clip);

    window.listen("the clip to load", is_loaded);
    let length = window
        .heard
        .iter()
        .find_map(|notice| {
            if let MediaNotice::Player(PlayerEvent::Loaded { length }) = notice {
                Some(length.0.as_millis())
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        (2900..=3200).contains(&length),
        "the clip is 3 s long: {length} ms"
    );
    assert!(
        window
            .started
            .facts
            .rows()
            .iter()
            .any(|row| row.label == anyview_core::FactLabel::Duration),
        "the host probed its length for the Info tab"
    );

    // The picture the player draws is what the window's texture shows.
    eventually("a frame in the window's texture", || {
        window
            .texture
            .size()
            .is_some_and(|(w, h)| (w.0, h.0) == (64, 48))
    });

    // The desktop's one entry follows it.
    eventually("the entry to say playing", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Playing)
    });
    let entry = last_state(&rig.desk).unwrap();
    assert_eq!(
        entry.title.as_deref(),
        Some("clip.mkv"),
        "no tag: the file's name"
    );
    assert_eq!(entry.file.as_ref(), Some(&clip));
    assert!(
        entry
            .length
            .is_some_and(|length| length.0.as_millis() > 2900)
    );

    // The desktop's controls reach the player, and the window hears the result.
    rig.desk.press(MediaControl::Pause);
    window.listen("the pause", |notices| {
        notices.iter().any(|notice| {
            matches!(
                notice,
                MediaNotice::Player(PlayerEvent::Playback(Pace::Paused))
            )
        })
    });
    eventually("the entry to say paused", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Paused)
    });
    rig.desk
        .press(MediaControl::SeekTo(MediaTime::from_secs(2)));
    window.listen("the seek", |notices| {
        notices
            .iter()
            .any(|notice| matches!(notice, MediaNotice::Player(PlayerEvent::SeekDone)))
    });
    rig.desk
        .press(MediaControl::SetVolume(Volume::clamped(Percent(30))));
    window.listen("the volume", |notices| {
        notices.iter().any(|notice| {
            matches!(
                notice,
                MediaNotice::Player(PlayerEvent::VolumeChanged(volume)) if volume.percent() == Percent(30)
            )
        })
    });
    eventually("the entry to say the volume", || {
        last_state(&rig.desk).is_some_and(|state| state.volume.percent() == Percent(30))
    });
    // Stop in a window holds at the start.
    rig.desk.press(MediaControl::Stop);
    eventually("the entry to be back at the start", || {
        last_state(&rig.desk).is_some_and(|state| {
            state.position.as_millis() < 700 && state.status == PlaybackStatus::Paused
        })
    });

    // Letting the window go ends the player, and the entry says stopped.
    drop(window);
    eventually("the entry to say stopped once the window let go", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Stopped)
    });
}

#[test]
fn a_session_with_no_window_plays_is_stopped_from_the_entry_and_goes() {
    let rig = rig();
    let tone = media_fixture("tone.flac");
    if let Err(error) = rig.hub.play_in_background(&tone, &Resume::Nothing) {
        eprintln!("SKIPPED: cannot start a background session ({error})");
        return;
    }
    assert!(rig.hub.plays_in_background());
    eventually("the entry to say playing", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Playing)
    });
    assert_eq!(
        last_state(&rig.desk).unwrap().title.as_deref(),
        Some("tone.flac")
    );
    rig.desk.press(MediaControl::Pause);
    eventually("the entry to say paused", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Paused)
    });
    rig.desk.press(MediaControl::Play);
    eventually("it to play again", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Playing)
    });
    rig.desk.press(MediaControl::Stop);
    eventually("the session to end", || !rig.hub.plays_in_background());
    eventually("the entry to say stopped", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Stopped)
    });
}

#[test]
fn a_session_with_no_window_ends_when_its_recording_does() {
    let rig = rig();
    let tone = media_fixture("tone.flac");
    if let Err(error) = rig.hub.play_in_background(&tone, &Resume::Nothing) {
        eprintln!("SKIPPED: cannot start a background session ({error})");
        return;
    }
    assert!(rig.hub.plays_in_background());
    eventually("the two second tone to end and its session with it", || {
        !rig.hub.plays_in_background()
    });
    eventually("the entry to say stopped", || {
        last_state(&rig.desk).is_some_and(|state| state.status == PlaybackStatus::Stopped)
    });
}

#[test]
fn a_background_session_starts_where_the_file_was_left() {
    let rig = rig();
    let clip = media_fixture("clip.mkv");
    let left = Resume::Media {
        at: MediaTime::from_millis(1500),
        volume: Volume::FULL,
        audio: anyview_core::TrackChoice::Auto,
        subtitles: anyview_core::TrackChoice::Auto,
    };
    if let Err(error) = rig.hub.play_in_background(&clip, &left) {
        eprintln!("SKIPPED: cannot start a background session ({error})");
        return;
    }
    eventually("a position at or after where it was left", || {
        last_state(&rig.desk).is_some_and(|state| state.position.as_millis() >= 1400)
    });
    rig.hub.stop_background();
    eventually("the session to end", || !rig.hub.plays_in_background());
}

fn desktop(rig: &Rig, scratch: &std::path::Path) -> Arc<dyn Hosting> {
    let now: Clock = Arc::new(|| Viewed(1_700_000_000));
    Arc::new(Desktop::new(
        rig.runtime.handle().clone(),
        FakeApps::default(),
        FakeReveal::default(),
        FakeShare::default(),
        FakePrinter::answering(PrintOutcome::Printed),
        NoTrash,
        Services {
            store: Store::new(&scratch.join("store"), now),
            media: Media {
                hub: rig.hub.clone(),
                exports: rig.workforce.exports(),
                scratch: scratch.join("cache"),
            },
        },
    ))
}

fn probed(file: &FilePath) -> Probed {
    let bytes = std::fs::read(file.as_path()).unwrap();
    let name = FileName::new(file.file_name().unwrap().as_str()).unwrap();
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(&bytes[..bytes.len().min(4096)]), &name)
    else {
        panic!("a recording needs no look inside");
    };
    let family: StageFamily = family_of(sniffed.kind());
    Probed {
        source: Source::new(
            file.clone(),
            FileStamp {
                len: ByteLen(bytes.len() as u64),
                modified: ModTime(1),
            },
        ),
        sniffed,
        family,
        resume: Resume::Nothing,
    }
}

/// `name` copied into `dir`, so what an export writes beside it lands in the scratch folder.
fn copy_into(dir: &std::path::Path, name: &str) -> FilePath {
    let to = dir.join(name);
    std::fs::copy(media_fixture(name).as_path(), &to).unwrap();
    FilePath::new(std::fs::canonicalize(to).unwrap()).unwrap()
}

#[test]
fn a_cut_and_a_track_are_written_beside_the_recording() {
    let rig = rig();
    let dir = tempfile::tempdir().unwrap();
    let hosting = desktop(&rig, dir.path());
    let clip = copy_into(dir.path(), "clip.mkv");
    let trim = MediaExport::Trim(
        TimeRange::new(
            MediaTime::from_millis(1500),
            Some(MediaTime::from_millis(2500)),
        )
        .unwrap(),
    );
    let outcome = rig
        .runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(&clip),
            choice: trim,
        }))
        .unwrap();
    assert_eq!(outcome, Outcome::Done);
    let cut = dir.path().join("clip trimmed.mkv");
    assert!(
        cut.exists(),
        "{:?}",
        std::fs::read_dir(dir.path()).unwrap().collect::<Vec<_>>()
    );
    let probe = anyview_media::probe(&cut).unwrap();
    let length = probe.length.unwrap().0.as_millis();
    assert!(
        (900..=1800).contains(&length),
        "a keyframe-aligned cut about a second long: {length} ms"
    );

    let tone = copy_into(dir.path(), "tone.flac");
    let outcome = rig
        .runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(&tone),
            choice: MediaExport::AudioOnly(anyview_core::AudioTarget::Wav),
        }))
        .unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert!(dir.path().join("tone.wav").exists());
}

#[test]
fn the_frame_on_screen_is_saved_at_the_pictures_own_size_in_the_format_asked_for() {
    let Some(gpu) = gpu() else {
        return;
    };
    let rig = rig();
    let dir = tempfile::tempdir().unwrap();
    let hosting = desktop(&rig, dir.path());
    let clip = copy_into(dir.path(), "clip.mkv");
    let host = PlayerHost::new(rig.hub.clone());
    let mut window = Window::open(&host, &gpu, &clip);
    window.listen("the clip to load", is_loaded);
    eventually("a frame in the window's texture", || {
        window.texture.size().is_some()
    });

    let png = rig
        .runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(&clip),
            choice: MediaExport::CurrentFrame(RasterTarget::Png),
        }))
        .unwrap();
    assert_eq!(png, Outcome::Done);
    let bytes = std::fs::read(dir.path().join("clip frame.png")).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let size = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(
        (size(16), size(20)),
        (64, 48),
        "the clip's own size, not the window's"
    );

    let jpeg = rig
        .runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(&clip),
            choice: MediaExport::CurrentFrame(RasterTarget::default_jpeg()),
        }))
        .unwrap();
    assert_eq!(jpeg, Outcome::Done);
    let bytes = std::fs::read(dir.path().join("clip frame.jpg")).unwrap();
    assert_eq!(&bytes[..2], b"\xff\xd8", "a JPEG");
    let again = rig
        .runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(&clip),
            choice: MediaExport::CurrentFrame(RasterTarget::Png),
        }))
        .unwrap();
    assert_eq!(again, Outcome::Done);
    assert!(
        dir.path().join("clip frame 2.png").exists(),
        "the second is under a free name, the first is kept"
    );
}

#[test]
fn a_frame_of_a_recording_no_window_plays_is_a_failure_not_a_hang() {
    let rig = rig();
    let dir = tempfile::tempdir().unwrap();
    let hosting = desktop(&rig, dir.path());
    let clip = copy_into(dir.path(), "clip.mkv");
    let outcome = rig
        .runtime
        .block_on(hosting.carry_out(Task::ExportMedia {
            file: probed(&clip),
            choice: MediaExport::CurrentFrame(RasterTarget::Png),
        }))
        .unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
}
