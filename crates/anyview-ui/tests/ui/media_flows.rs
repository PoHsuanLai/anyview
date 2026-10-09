//! What a person does around a recording: marks for a trim, a file dropped over a playing one, and
//! quick repeats of the play key.

use crate::support;

use anyview_core::{MediaExport, MediaLength, MediaTime, TimeRange, VideoPresence};
use anyview_ui::{ExportDraft, HostRequest, MediaNotice, PlayerCommand, PlayerEvent};
use ds::file_drop::drag::{FileDragInput, Offer};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Viewport};
use std::path::PathBuf;
use std::sync::Arc;
use support::{Answer, FakeLine, FakePlayer, Requests, Wiring, folder, middle, settle, wired};

const SCALES: [u16; 2] = [100, 200];
const LENGTH: MediaLength = MediaLength(MediaTime::from_secs(100));
const FILES: &[(&str, &str, &str)] = &[
    ("anyview-media", "clip.mkv", "1-clip.mkv"),
    ("anyview-text", "notes.txt", "2-notes.txt"),
];

struct Opened {
    _dir: tempfile::TempDir,
    paths: Vec<PathBuf>,
    harness: Harness,
    requests: Requests,
    player: Arc<FakePlayer>,
}

fn open(scale: u16) -> Opened {
    let (dir, paths) = folder(FILES);
    let player = FakePlayer::answering(Answer::Plays);
    let wiring = Wiring {
        player: Some(Arc::clone(&player)),
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        ..Wiring::default()
    };
    let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    Opened {
        _dir: dir,
        paths,
        harness,
        requests,
        player,
    }
}

fn say_at(line: &FakeLine, seconds: u64) {
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH }),
        MediaNotice::Position(MediaTime::from_secs(seconds)),
        MediaNotice::Picture(VideoPresence::Present),
    ]);
}

fn key(harness: &mut Harness, key: ShortcutKey) {
    harness.send(Input::key(key));
    settle(harness);
}

/// Mark the start at `from` and the end at `to` seconds, in the order given.
fn mark(harness: &mut Harness, player: &FakePlayer, marks: &[(char, u64)]) {
    for (letter, at) in marks {
        say_at(&player.latest().unwrap(), *at);
        settle(harness);
        key(harness, ShortcutKey::Char(*letter));
    }
}

/// Export the trim from the palette, as the sheet offers it, and the trim the host is asked for.
fn exported_trim(harness: &mut Harness, requests: &Requests) -> Option<TimeRange> {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    for letter in "export".chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(harness);
    key(harness, ShortcutKey::Enter);
    harness.send(Input::click(
        harness
            .centre(".viewer-sheet .ds-list-item:nth-child(6)")
            .unwrap(),
    ));
    settle(harness);
    harness.send(Input::click(
        harness
            .centre(".viewer-sheet-buttons .ds-button:nth-child(2)")
            .unwrap(),
    ));
    settle(harness);
    requests.lock().unwrap().iter().find_map(|request| {
        if let HostRequest::Export(ExportDraft::Media(MediaExport::Trim(range))) = request {
            Some(*range)
        } else {
            None
        }
    })
}

/// A recording's marks, in two steps (each opens the recording afresh): the marks of an earlier
/// visit are gone when it is opened again, and an end mark before the start mark does not export
/// the whole recording quietly.
#[test]
fn marks_belong_to_one_visit_and_a_backwards_pair_is_not_the_whole_recording() {
    for scale in SCALES {
        {
            let Opened {
                mut harness,
                player,
                requests,
                _dir,
                ..
            } = open(scale);
            say_at(&player.latest().unwrap(), 10);
            settle(&mut harness);
            mark(&mut harness, &player, &[('i', 10)]);
            key(&mut harness, ShortcutKey::Right);
            key(&mut harness, ShortcutKey::Left);
            settle(&mut harness);
            mark(&mut harness, &player, &[('o', 25)]);
            let range = exported_trim(&mut harness, &requests);
            assert_eq!(
                range,
                Some(
                    TimeRange::new(MediaTime::from_secs(0), Some(MediaTime::from_secs(25)))
                        .unwrap()
                ),
                "step marks of an earlier visit, {scale}: the start mark of the earlier visit is not this visit's"
            );
        }
        {
            let Opened {
                mut harness,
                player,
                requests,
                _dir,
                ..
            } = open(scale);
            say_at(&player.latest().unwrap(), 10);
            settle(&mut harness);
            mark(&mut harness, &player, &[('i', 40), ('o', 20)]);
            let range = exported_trim(&mut harness, &requests);
            assert_ne!(
                range,
                Some(TimeRange::WHOLE),
                "step end before start, {scale}: start 40s, end 20s: the cut is not the whole recording"
            );
        }
    }
}

#[test]
fn a_file_dropped_on_a_playing_recording_ends_its_player() {
    for scale in SCALES {
        let Opened {
            mut harness,
            player,
            paths,
            _dir,
            ..
        } = open(scale);
        assert_eq!(player.alive().len(), 1);
        for step in [
            FileDragInput::Entered {
                point: Some(middle()),
            },
            FileDragInput::Offered(Offer::Files(vec![paths[1].clone()])),
            FileDragInput::Moved { point: middle() },
            FileDragInput::Dropped,
        ] {
            harness.send(Input::FileDrag(step));
        }
        settle(&mut harness);
        settle(&mut harness);
        assert!(
            player.alive().is_empty(),
            "{scale}: nothing plays over a text"
        );
    }
}

#[test]
fn space_pressed_twice_quickly_is_a_play_and_a_pause() {
    for scale in SCALES {
        let Opened {
            mut harness,
            player,
            _dir,
            ..
        } = open(scale);
        let line = player.latest().unwrap();
        say_at(&line, 5);
        settle(&mut harness);
        let before = line.sent().len();
        harness.send(Input::key(ShortcutKey::Space));
        harness.send(Input::key(ShortcutKey::Space));
        settle(&mut harness);
        let sent: Vec<PlayerCommand> = line.sent()[before..].to_vec();
        assert_eq!(sent.len(), 2, "{scale}: {sent:?}");
        assert_ne!(sent[0], sent[1], "{scale}: one plays, one pauses: {sent:?}");
    }
}
