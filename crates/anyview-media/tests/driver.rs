//! The driver on the fixtures with a real player: events in the order the viewer needs them, the
//! lists a loaded recording has, a seek that lands, the end of a file mpv holds open, a saved
//! frame, and an audio file's cover. Sound is off (`ao=null`) and the device has no window.
//! They skip, saying so, on a machine with no graphics adapter.

#![cfg(feature = "player")]
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{
    ChapterIndex, MediaTime, Speed, StreamKind, TrackChoice, TrackId, TrackPlay, VideoPresence,
    Volume,
};
use anyview_media::{
    Continuation, Direction, EndReason, MediaCommand, MediaEvent, Pace, PictureSlot, ShotContent,
};
use std::num::NonZeroU32;
use support::{Rig, device, fixture};

fn slot(width: u32, height: u32) -> MediaCommand {
    MediaCommand::Slot(PictureSlot::Sized {
        width: NonZeroU32::new(width).unwrap(),
        height: NonZeroU32::new(height).unwrap(),
    })
}

fn loaded(events: &[MediaEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, MediaEvent::Loaded { .. }))
}

#[test]
fn a_loaded_recording_says_what_it_has_in_the_order_the_viewer_needs_it() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    rig.until("the file to load", loaded);
    // The child process lists the chapters a moment after the file is loaded.
    rig.until("the chapters", |events| {
        support::last_chapters(events).is_some_and(|count| count >= 1)
    });

    let at = |wanted: fn(&MediaEvent) -> bool| rig.events.iter().position(wanted).unwrap();
    let opened = at(|event| matches!(event, MediaEvent::Loaded { .. }));
    // mpv announces tracks as it opens the file, before it says the file is loaded; what the
    // driver adds once it is loaded comes after.
    let last = |wanted: fn(&MediaEvent) -> bool| rig.events.iter().rposition(wanted).unwrap();
    assert!(
        opened < last(|event| matches!(event, MediaEvent::Tracks(_))),
        "the tracks are said again once it is loaded"
    );
    assert!(opened < last(|event| matches!(event, MediaEvent::Chapters(_))));
    assert!(opened < last(|event| matches!(event, MediaEvent::Volume(_))));
    rig.until("the length", |events| support::length_of(events).is_some());
    let said = |wanted: fn(&MediaEvent) -> bool| rig.events.iter().filter(|e| wanted(e)).count();
    assert_eq!(
        said(|event| matches!(event, MediaEvent::Length(_)))
            + said(|event| matches!(event, MediaEvent::Loaded { length: Some(_) })),
        1,
        "the length is said once, with `Loaded` or after it: {:?}",
        rig.events
    );
    let length = support::length_of(&rig.events).unwrap().0.as_millis();
    assert!((2900..=3200).contains(&length), "3.03 s, not {length} ms");

    let tracks = support::last_tracks(&rig.events).unwrap();
    let kinds = |kind: StreamKind| tracks.iter().filter(|track| track.kind == kind).count();
    assert_eq!(kinds(StreamKind::Video), 1);
    assert_eq!(kinds(StreamKind::Audio), 2, "two audio tracks");
    assert_eq!(kinds(StreamKind::Subtitles), 1, "one subtitle track");
    let chapters = support::last_chapters(&rig.events).unwrap();
    assert!(chapters >= 1, "the clip has chapters");
    assert!(
        rig.events
            .contains(&MediaEvent::Picture(VideoPresence::Present))
    );
    assert!(rig.events.contains(&MediaEvent::Volume(Volume::FULL)));
    assert!(rig.events.contains(&MediaEvent::Speed(Speed::NORMAL)));
}

#[test]
fn instructions_sent_before_the_file_opens_wait_and_run_in_order_when_it_does() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    // Nothing has been polled: the file is not open.
    rig.send(MediaCommand::SetVolume(Volume::SILENT));
    rig.send(MediaCommand::SetSpeed(Speed::from_thousandths(1500)));
    assert!(!loaded(&rig.events), "nothing ran yet");
    rig.until("the speed to be reported", |events| {
        events.contains(&MediaEvent::Speed(Speed::from_thousandths(1500)))
    });
    rig.until("the volume to change", |events| {
        events.contains(&MediaEvent::Volume(Volume::SILENT))
    });
}

#[test]
fn a_seek_lands_and_the_position_follows_it() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    rig.until("the file to load", loaded);
    rig.send(MediaCommand::SetPlayback(Pace::Paused));
    rig.send(MediaCommand::Seek(MediaTime::from_secs(2)));
    rig.until("the seek to land", |events| {
        events.contains(&MediaEvent::SeekDone)
    });
    rig.until("a position near the target", |events| {
        support::position_within(events, 1_800_000..=2_300_000)
    });
    let landed = rig
        .events
        .iter()
        .position(|event| *event == MediaEvent::SeekDone)
        .unwrap();
    let near = support::position_within(&rig.events[landed..], 1_800_000..=2_300_000);
    assert!(
        near,
        "a position near the target is reported after the seek lands"
    );
}

#[test]
fn a_file_mpv_holds_open_at_its_end_is_reported_as_ended() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("tone.flac"));
    rig.until("the end of a two second tone", |events| {
        events.contains(&MediaEvent::Ended(EndReason::Eof))
    });
    let paused = rig
        .events
        .iter()
        .position(|event| *event == MediaEvent::Playback(Pace::Paused))
        .unwrap();
    let ended = rig
        .events
        .iter()
        .position(|event| *event == MediaEvent::Ended(EndReason::Eof))
        .unwrap();
    assert!(paused < ended, "mpv paused, then the driver said it ended");
    // The child process reports the last position after the pause; said after the end it would
    // read as the file playing again, so nothing is said of the position until it moves again.
    for _ in 0..10 {
        let _ = rig.wakes.recv_timeout(std::time::Duration::from_millis(50));
        let heard = rig.driver.woken();
        rig.events.extend(heard);
    }
    assert!(
        !rig.events[ended..]
            .iter()
            .any(|event| matches!(event, MediaEvent::Position(_))),
        "{:?}",
        &rig.events[ended..]
    );
}

#[test]
fn an_audio_file_with_no_picture_shows_none_and_its_cover_shows_as_a_still() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut plain = Rig::open(&device, &queue, &fixture("tone.flac"));
    plain.send(slot(64, 64));
    plain.until("the file to load", loaded);
    assert!(plain.has(|event| *event == MediaEvent::Picture(VideoPresence::Absent)));
    assert_eq!(plain.shown.seen().textures, 0, "no picture, no texture");

    let mut covered = Rig::open(&device, &queue, &fixture("cover.mp3"));
    covered.send(slot(64, 64));
    covered.until("the cover to be shown", |events| {
        events.contains(&MediaEvent::Picture(VideoPresence::CoverArt))
    });
    let seen = covered.shown.clone();
    covered.until("a frame", |_| seen.drawn() > 0);
    assert!(covered.shown.seen().textures >= 1, "the cover is a texture");
}

#[test]
fn a_new_slot_size_announces_a_new_texture_and_a_frame_redraws_the_same_one() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    let seen = rig.shown.clone();
    rig.send(slot(64, 48));
    rig.until("the first frame", |_| seen.seen().textures >= 1);
    let first = seen.seen().textures;
    rig.until("more frames of that texture", |_| seen.seen().frames >= 3);
    assert_eq!(
        rig.shown.seen().textures,
        first,
        "frames in the same slot reuse the texture"
    );
    rig.send(slot(96, 72));
    let before = rig.shown.seen().textures;
    rig.until("a texture for the new size", |_| {
        seen.seen().textures > before
    });
}

#[test]
fn track_chapter_and_speed_instructions_are_carried_out() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    rig.until("the file to load", loaded);
    // The child process lists the chapters a moment after the file is loaded; a chapter can only
    // be gone to once they are known.
    rig.until("the chapters", |events| {
        support::last_chapters(events).is_some_and(|count| count >= 1)
    });
    rig.send(MediaCommand::SelectTrack {
        kind: StreamKind::Audio,
        choice: TrackChoice::Track(TrackId(2)),
    });
    rig.send(MediaCommand::CycleTrack(StreamKind::Subtitles));
    rig.send(MediaCommand::GoToChapter(ChapterIndex(0)));
    rig.send(MediaCommand::StepSpeed(Direction::Forward));
    rig.until(
        "the tracks to be reported with the second audio playing",
        |events| {
            support::last_tracks(events).is_some_and(|tracks| {
                tracks.iter().any(|track| {
                    track.kind == StreamKind::Audio
                        && track.id == TrackId(2)
                        && track.play == TrackPlay::Playing
                })
            })
        },
    );
    assert!(
        rig.events
            .contains(&MediaEvent::Speed(Speed::from_thousandths(1250)))
    );
    assert!(
        !rig.has(|event| matches!(event, MediaEvent::Refused(_))),
        "{:?}",
        rig.events
    );
}

#[test]
fn a_saved_frame_is_the_pictures_own_size_and_a_missing_folder_is_a_failure() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    let seen = rig.shown.clone();
    rig.send(slot(256, 192));
    rig.until("a frame", |_| seen.drawn() >= 1);
    let dir = tempfile::tempdir().unwrap();
    let to = anyview_core::FilePath::new(dir.path().join("frame.png")).unwrap();
    rig.send(MediaCommand::Screenshot {
        to: to.clone(),
        content: ShotContent::Video,
    });
    assert!(
        rig.has(|event| *event == MediaEvent::ShotSaved(to.clone())),
        "{:?}",
        rig.events
    );
    let bytes = std::fs::read(to.as_path()).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    // IHDR: width and height are the clip's own 64 by 48, not the 256 by 192 slot.
    let size = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!((size(16), size(20)), (64, 48));

    let missing = anyview_core::FilePath::new(dir.path().join("no/such/frame.png")).unwrap();
    rig.send(MediaCommand::Screenshot {
        to: missing.clone(),
        content: ShotContent::Video,
    });
    assert!(rig.has(|event| matches!(event, MediaEvent::ShotFailed { to, .. } if *to == missing)));
}

#[test]
fn a_file_that_will_not_open_fails_and_closing_ends_the_session() {
    let Some((device, queue)) = device() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let nothing = anyview_core::FilePath::new(dir.path().join("missing.mkv")).unwrap();
    let mut rig = Rig::open(&device, &queue, &nothing);
    rig.until("the failure", |events| {
        events
            .iter()
            .any(|event| matches!(event, MediaEvent::Failed(_)))
    });
    let handled = rig.driver.command(MediaCommand::Close);
    assert_eq!(handled.then, Continuation::Close);
}

#[test]
fn a_pause_the_host_asks_for_is_reported_though_mpv_says_nothing_of_it() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut rig = Rig::open(&device, &queue, &fixture("clip.mkv"));
    rig.until("the file to load", loaded);
    let before = rig.events.len();
    rig.send(MediaCommand::SetPlayback(Pace::Paused));
    assert_eq!(
        rig.events[before..],
        [MediaEvent::Playback(Pace::Paused)],
        "said at once, by the driver"
    );
    rig.send(MediaCommand::SetPlayback(Pace::Playing));
    assert_eq!(
        rig.events.last(),
        Some(&MediaEvent::Playback(Pace::Playing))
    );
}

/// A script that stands in for `mpv`: it records its own process id, then becomes the real one, so
/// a test can end the player the way a crash would.
fn recording_mpv(
    dir: &std::path::Path,
    real: &std::path::Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let pid = dir.join("mpv.pid");
    let script = dir.join("mpv");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec '{}' \"$@\"\n",
            pid.display(),
            real.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, pid)
}

#[test]
fn a_crashed_mpv_is_a_failed_event_and_never_a_panic() {
    let Some((device, queue)) = device() else {
        return;
    };
    let real = support::mpv_host().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (script, pid) = recording_mpv(dir.path(), &real.mpv);
    let host = anyview_media::MpvHost {
        mpv: script,
        cplugin: real.cplugin,
    };
    let mut rig = Rig::open_with(&device, &queue, &host, &fixture("clip.mkv"));
    rig.until("the file to load", loaded);
    let pid = std::fs::read_to_string(pid).unwrap();
    let killed = std::process::Command::new("kill")
        .args(["-9", pid.trim()])
        .status()
        .unwrap();
    assert!(killed.success(), "the player's process was found");
    rig.until("the failure", |events| {
        events
            .iter()
            .any(|event| matches!(event, MediaEvent::Failed(_)))
    });
    // The driver still answers: an instruction after the crash is refused, not fatal.
    rig.send(MediaCommand::SetPlayback(Pace::Paused));
    assert!(
        rig.has(|event| matches!(event, MediaEvent::Refused(reason) if reason.contains("mpv"))),
        "the refusal names the player: {:?}",
        rig.events
    );
    let handled = rig.driver.command(MediaCommand::Close);
    assert_eq!(handled.then, Continuation::Close);
}

#[test]
fn an_mpv_that_will_not_start_is_a_typed_error_naming_what_was_missing() {
    let Some((device, queue)) = device() else {
        return;
    };
    let real = support::mpv_host().unwrap();
    let host = anyview_media::MpvHost {
        mpv: "/nonexistent/mpv".into(),
        cplugin: real.cplugin,
    };
    let (tx, _wakes) = std::sync::mpsc::channel();
    let started = anyview_media::Driver::open(
        &device,
        &queue,
        anyview_media::AudioDriver::Null,
        &host,
        &fixture("clip.mkv"),
        Box::new(support::Recording::default()),
        move || {
            let _ = tx.send(());
        },
    );
    match started {
        Err(anyview_media::MediaError::PlayerStart(reason)) => {
            assert!(reason.contains("/nonexistent/mpv"), "{reason}");
        }
        Err(other) => panic!("another error: {other:?}"),
        Ok(_) => panic!("a player started with no mpv"),
    }
}
