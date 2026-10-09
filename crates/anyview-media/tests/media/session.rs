//! The typestate on a real player: an idle session is given a file and opens, only the loaded one
//! answers about the recording, a file that will not open gives the session back idle, and the
//! calls a loaded recording has do what they say. (That the other states do not have those calls
//! is the compile-fail example on `Session`.) Sound is off; the tests skip, saying so, on a machine
//! with no graphics adapter.

#![cfg(feature = "player")]

use crate::support;

use anyview_core::{
    ChapterIndex, MediaTime, Speed, StreamKind, TrackChoice, TrackId, TrackPlay, VideoPresence,
    Volume,
};
use anyview_media::{
    AudioDriver, Direction, Idle, Loaded, MediaEvent, Opened, Pace, PictureSlot, Session,
};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use support::{device, fixture};

/// An idle session on the test device.
fn idle() -> Option<Session<Idle>> {
    let (device, queue) = device()?;
    let host = support::mpv_host()?;
    Some(Session::new(&device, &queue, AudioDriver::Null, &host).unwrap())
}

/// Open `name` and poll until the player says so: the loaded session and what it said while
/// opening.
fn loaded(name: &str) -> Option<(Session<Loaded>, Vec<MediaEvent>)> {
    let mut opening = idle()?.open(&fixture(name)).unwrap();
    let started = Instant::now();
    let mut heard = Vec::new();
    loop {
        assert!(
            started.elapsed() < support::TIMEOUT,
            "never opened: {heard:?}"
        );
        match opening.poll() {
            Opened::Loaded(session, report) => {
                heard.extend(report.events);
                return Some((session, heard));
            }
            Opened::Waiting(session, report) => {
                heard.extend(report.events);
                opening = session;
                std::thread::sleep(Duration::from_millis(10));
            }
            Opened::Failed(_, report) => panic!("failed to open {name}: {:?}", report.events),
        }
    }
}

#[test]
fn an_idle_session_opens_a_file_and_becomes_a_loaded_one_that_answers_about_it() {
    let Some((session, heard)) = loaded("clip.mkv") else {
        return;
    };
    assert!(
        heard
            .iter()
            .any(|event| matches!(event, MediaEvent::Loaded { length: Some(_) })),
        "the load is announced with the length: {heard:?}"
    );
    let length = session.length().unwrap().0.as_millis();
    assert!((2900..=3200).contains(&length), "{length} ms");
    let tracks = session.tracks();
    assert_eq!(
        tracks
            .iter()
            .filter(|track| track.kind == StreamKind::Audio)
            .count(),
        2
    );
    let playing: Vec<u32> = tracks
        .iter()
        .filter(|track| track.kind == StreamKind::Audio && track.play == TrackPlay::Playing)
        .map(|track| track.id.0)
        .collect();
    assert_eq!(playing, vec![1], "the first audio track plays by default");
    assert!(!session.chapters().is_empty());
    assert_eq!(session.volume(), Volume::FULL);
    assert_eq!(session.speed(), Speed::NORMAL);
    assert_eq!(session.presence(), VideoPresence::Present);
    assert_eq!(session.pace(), Pace::Playing);
}

#[test]
fn a_file_that_will_not_open_gives_the_session_back_idle_with_the_reason() {
    let Some(idle) = idle() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let missing = anyview_core::FilePath::new(dir.path().join("nothing.mkv")).unwrap();
    let mut opening = idle.open(&missing).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < support::TIMEOUT);
        match opening.poll() {
            Opened::Failed(session, report) => {
                assert!(
                    report
                        .events
                        .iter()
                        .any(|event| matches!(event, MediaEvent::Failed(_))),
                    "{:?}",
                    report.events
                );
                // The session is idle again: it can be given another file.
                let again: Session<Idle> = session;
                let reopened = again.open(&fixture("tone.flac"));
                assert!(reopened.is_ok(), "an idle session takes another file");
                return;
            }
            Opened::Loaded(..) => panic!("a missing file opened"),
            Opened::Waiting(session, _) => {
                opening = session;
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[test]
fn the_calls_a_loaded_recording_has_change_what_it_reports() {
    let Some((mut session, _)) = loaded("clip.mkv") else {
        return;
    };
    session.set_slot(PictureSlot::Empty).unwrap();
    session.set_playback(Pace::Paused).unwrap();
    session.set_volume(Volume::SILENT).unwrap();
    session.set_speed(Speed::from_thousandths(2000)).unwrap();
    assert_eq!(session.speed(), Speed::from_thousandths(2000));
    session.step_speed(Direction::Backward).unwrap();
    assert_eq!(
        session.speed(),
        Speed::from_thousandths(1500),
        "a step goes to the next preset"
    );
    session
        .select_track(StreamKind::Audio, TrackChoice::Track(TrackId(2)))
        .unwrap();
    session.cycle_track(StreamKind::Subtitles).unwrap();
    session.go_to_chapter(ChapterIndex(0)).unwrap();
    session.seek(MediaTime::from_secs(1)).unwrap();

    // What the player reports follows once it has been polled.
    let started = Instant::now();
    let mut heard = Vec::new();
    while started.elapsed() < support::TIMEOUT {
        heard.extend(session.poll().events);
        let second_audio = session.tracks().iter().any(|track| {
            track.kind == StreamKind::Audio
                && track.id == TrackId(2)
                && track.play == TrackPlay::Playing
        });
        let subtitles_on = session
            .tracks()
            .iter()
            .any(|track| track.kind == StreamKind::Subtitles && track.play == TrackPlay::Playing);
        if second_audio && subtitles_on && session.volume() == Volume::SILENT {
            assert_eq!(session.pace(), Pace::Paused);
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the settings never showed: {heard:?}");
}

#[test]
fn a_new_slot_gives_the_session_a_picture_to_show() {
    let Some((mut session, _)) = loaded("clip.mkv") else {
        return;
    };
    assert!(session.picture().is_none(), "no slot, no picture");
    session
        .set_slot(PictureSlot::Sized {
            width: NonZeroU32::new(64).unwrap(),
            height: NonZeroU32::new(48).unwrap(),
        })
        .unwrap();
    let started = Instant::now();
    while session.picture().is_none() {
        assert!(started.elapsed() < support::TIMEOUT, "no picture came");
        session.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
}
