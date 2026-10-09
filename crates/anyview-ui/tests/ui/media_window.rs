//! The viewer window on a recording, under the harness on the virtual clock, with a player that
//! is a script: the window starts it, the test says what it reports and reads what the window
//! told it. What the machines decide is in their tables; these check that the window carries it:
//! the capsule's controls, the scrubber's drag, the keys, where the person is kept and put back,
//! the lists in the panel, the trim marks of an export, and that a player ends with its window.

use crate::support;

use anyview_core::{
    ChapterIndex, Fact, FactLabel, FactValue, MediaChapter, MediaExport, MediaExportKind,
    MediaLength, MediaTags, MediaTime, MediaTrack, Percent, Resume, Speed, StreamKind, TimeRange,
    TrackChoice, TrackId, TrackPlay, VideoPresence, Volume,
};
use anyview_ui::{
    ExportDraft, HostRequest, MediaError, MediaNotice, MediaOffer, Pace, PlayerCommand,
    PlayerEvent, Presentation, StepDirection, TrackKind,
};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use std::sync::Arc;
use support::{
    Answer, FakeLine, FakePlayer, Memory, Requests, Wiring, folder, settle, shot, wired,
};

const FILES: &[(&str, &str, &str)] = &[
    ("anyview-media", "clip.mkv", "1-clip.mkv"),
    ("anyview-media", "tone.flac", "2-tone.flac"),
    ("anyview-text", "notes.txt", "3-notes.txt"),
];

const LENGTH: MediaLength = MediaLength(MediaTime::from_secs(100));

struct Opened {
    _dir: tempfile::TempDir,
    paths: Vec<PathBuf>,
    harness: Harness,
    requests: Requests,
    player: Arc<FakePlayer>,
    memory: Arc<Memory>,
}

fn open_with(at: usize, wiring: impl FnOnce(&[PathBuf]) -> Wiring) -> Opened {
    let (dir, paths) = folder(FILES);
    let player = FakePlayer::answering(Answer::Plays);
    let memory = Arc::new(Memory::default());
    let mut wiring = wiring(&paths);
    wiring.player = Some(Arc::clone(&player));
    wiring.memory.get_or_insert_with(|| Arc::clone(&memory));
    let memory = wiring.memory.clone().unwrap();
    let (harness, requests, _) = wired(&paths, at, Appearance::default(), wiring);
    Opened {
        _dir: dir,
        paths,
        harness,
        requests,
        player,
        memory,
    }
}

fn open() -> Opened {
    open_with(0, |_| Wiring::default())
}

fn middle() -> Point {
    Point {
        x: Px(450.0),
        y: Px(300.0),
    }
}

fn playing(line: &FakeLine, at: u64) {
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH }),
        MediaNotice::Position(MediaTime::from_secs(at)),
        MediaNotice::Picture(VideoPresence::Present),
    ]);
}

/// The window drawn with the pointer on it, so the capsule shows.
fn hover(harness: &mut Harness) {
    harness.send(Input::pointer_move(middle()));
    settle(harness);
}

fn readouts(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

fn track(kind: StreamKind, id: u32, title: &str, play: TrackPlay) -> MediaTrack {
    MediaTrack {
        id: TrackId(id),
        kind,
        title: Some(title.to_owned()),
        language: None,
        codec: None,
        play,
    }
}

#[test]
fn a_recording_starts_a_player_and_its_news_brings_the_capsule_to_life() {
    let Opened {
        mut harness,
        player,
        paths,
        ..
    } = open();
    let line = player.latest().expect("the window started a player");
    assert_eq!(line.file.as_path(), paths[0].as_path());
    assert_eq!(
        player.starts(),
        1,
        "only the open file has a player: preloading the next recording would play it unseen"
    );
    assert!(
        harness
            .text_of(".viewer-media-status")
            .is_some_and(|word| word == "Opening"),
        "the window says it is opening until the player says otherwise"
    );

    playing(&line, 25);
    hover(&mut harness);
    let text = readouts(&harness);
    assert!(text.contains("0:25"), "the position: {text}");
    assert!(text.contains("1:40"), "the length: {text}");
    assert!(text.contains("1×"), "the speed: {text}");
    assert_eq!(harness.count(".ds-scrubber"), 1, "the progress bar");
    assert_eq!(harness.count(".ds-capsule .ds-slider"), 1, "the volume");
    assert_eq!(
        harness.count(".viewer-media-status"),
        0,
        "nothing left to say"
    );
    assert!(
        line.slots().iter().any(Option::is_some),
        "the player was told how large the picture is: {:?}",
        line.slots()
    );
}

#[test]
fn space_and_the_play_button_toggle_playback_and_the_player_hears_both() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 10);
    hover(&mut harness);

    harness.send(Input::key(ShortcutKey::Space));
    settle(&mut harness);
    assert_eq!(
        line.sent(),
        vec![PlayerCommand::SetPlayback(Pace::Paused)],
        "space pauses"
    );
    // The play button is the second of the capsule's buttons.
    harness.send(Input::click(
        harness
            .centre(".ds-capsule .ds-button:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(
        line.sent(),
        vec![
            PlayerCommand::SetPlayback(Pace::Paused),
            PlayerCommand::SetPlayback(Pace::Playing)
        ],
        "the button plays again"
    );
}

#[test]
fn a_position_the_player_reports_moves_the_clock_and_the_bar_and_a_pause_it_reports_is_followed() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 20);
    hover(&mut harness);
    let before = harness.attr(".ds-scrubber", "aria-valuenow");
    line.say(&[MediaNotice::Position(MediaTime::from_secs(60))]);
    settle(&mut harness);
    assert!(readouts(&harness).contains("1:00"));
    assert_ne!(
        harness.attr(".ds-scrubber", "aria-valuenow"),
        before,
        "the bar moved"
    );

    line.say(&[MediaNotice::Player(PlayerEvent::Playback(Pace::Paused))]);
    settle(&mut harness);
    harness.send(Input::key(ShortcutKey::Space));
    settle(&mut harness);
    assert_eq!(
        line.sent(),
        vec![PlayerCommand::SetPlayback(Pace::Playing)],
        "it was paused by someone else, so the next toggle plays"
    );
}

#[test]
fn dragging_the_scrubber_holds_the_player_seeks_live_and_lets_go_where_it_ends() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 10);
    hover(&mut harness);

    let track = harness.rect(".ds-scrubber-track").unwrap();
    let at = |share: f32| Point {
        x: Px(track.origin.x.0 + track.size.width.0 * share),
        y: Px(track.origin.y.0 + track.size.height.0 / 2.0),
    };
    harness.send(Input::drag(at(0.2), at(0.8), 6));
    settle(&mut harness);

    let sent = line.sent();
    assert_eq!(
        sent.first(),
        Some(&PlayerCommand::SetPlayback(Pace::Paused)),
        "grabbing the bar holds the player: {sent:?}"
    );
    let seeks: Vec<MediaTime> = sent
        .iter()
        .filter_map(|command| {
            if let PlayerCommand::Seek(to) = command {
                Some(*to)
            } else {
                None
            }
        })
        .collect();
    assert!(seeks.len() >= 3, "the player follows the drag: {sent:?}");
    assert!(
        seeks.windows(2).all(|pair| pair[0] <= pair[1]),
        "going right only moves later: {seeks:?}"
    );
    let end = seeks.last().unwrap().0 / 1_000_000;
    assert!(
        (70..=90).contains(&end),
        "it lets go near 80 s of 100, at {end} s"
    );
    assert_eq!(
        sent.last(),
        Some(&PlayerCommand::SetPlayback(Pace::Playing)),
        "it was playing, so it plays on from there: {sent:?}"
    );
}

#[test]
fn a_place_left_is_put_back_and_the_place_the_person_is_at_is_kept() {
    let place = Resume::Media {
        at: MediaTime::from_secs(25),
        volume: Volume::clamped(Percent(80)),
        audio: TrackChoice::Track(TrackId(2)),
        subtitles: TrackChoice::Off,
    };
    let Opened {
        mut harness,
        player,
        memory,
        paths,
        ..
    } = open_with(0, |paths| Wiring {
        memory: Some(Memory::with(&paths[0], place.clone())),
        ..Wiring::default()
    });
    let line = player.latest().unwrap();
    assert_eq!(
        line.sent(),
        vec![
            PlayerCommand::Seek(MediaTime::from_secs(25)),
            PlayerCommand::SetVolume(Volume::clamped(Percent(80))),
            PlayerCommand::SelectTrack {
                kind: TrackKind::Audio,
                choice: TrackChoice::Track(TrackId(2)),
            },
            PlayerCommand::SelectTrack {
                kind: TrackKind::Subtitles,
                choice: TrackChoice::Off,
            },
        ],
        "told before the file opened, in the order the player should hear them"
    );

    // Playing on: the place is kept once it has moved a second or more.
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH }),
        MediaNotice::Player(PlayerEvent::VolumeChanged(Volume::clamped(Percent(80)))),
        MediaNotice::Tracks(vec![
            track(StreamKind::Audio, 1, "Main", TrackPlay::Idle),
            track(StreamKind::Audio, 2, "Commentary", TrackPlay::Playing),
            track(StreamKind::Subtitles, 1, "English", TrackPlay::Idle),
        ]),
        MediaNotice::Position(MediaTime::from_secs(25)),
    ]);
    settle(&mut harness);
    line.say(&[MediaNotice::Position(MediaTime::from_secs(42))]);
    settle(&mut harness);
    assert_eq!(
        memory.left_at(&paths[0]),
        Some(Resume::Media {
            at: MediaTime::from_secs(42),
            volume: Volume::clamped(Percent(80)),
            audio: TrackChoice::Track(TrackId(2)),
            subtitles: TrackChoice::Off,
        }),
        "the position, the volume and the tracks"
    );
}

#[test]
fn a_position_that_only_creeps_is_not_kept_again_and_the_start_does_not_overwrite_a_place() {
    let place = Resume::Media {
        at: MediaTime::from_secs(60),
        volume: Volume::FULL,
        audio: TrackChoice::Auto,
        subtitles: TrackChoice::Auto,
    };
    let Opened {
        mut harness,
        player,
        memory,
        requests,
        paths,
        ..
    } = open_with(0, |paths| Wiring {
        memory: Some(Memory::with(&paths[0], place.clone())),
        ..Wiring::default()
    });
    let line = player.latest().unwrap();
    // While it is still opening nothing is kept, so a player reporting the start of the file
    // before its seek lands cannot overwrite the place that was left.
    line.say(&[MediaNotice::Position(MediaTime::from_secs(0))]);
    settle(&mut harness);
    assert_eq!(
        memory.left_at(&paths[0]),
        Some(place.clone()),
        "still where it was left"
    );

    line.say(&[MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH })]);
    settle(&mut harness);
    let remembered = |requests: &Requests| {
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| matches!(request, HostRequest::Remember(_)))
            .count()
    };
    line.say(&[MediaNotice::Position(MediaTime::from_secs(60))]);
    settle(&mut harness);
    let first = remembered(&requests);
    line.say(&[MediaNotice::Position(MediaTime::from_millis(60_300))]);
    settle(&mut harness);
    assert_eq!(remembered(&requests), first, "300 ms of creep says nothing");
    line.say(&[MediaNotice::Position(MediaTime::from_secs(63))]);
    settle(&mut harness);
    assert_eq!(remembered(&requests), first + 1, "three seconds do");
}

#[test]
fn walking_to_another_file_ends_the_player_and_walking_back_starts_another() {
    let Opened {
        mut harness,
        player,
        _dir,
        ..
    } = open();
    assert_eq!(player.alive().len(), 1);
    harness.send(Input::pointer_move(middle()));
    settle(&mut harness);
    // The second file is audio too: a player for it, and none left for the first.
    harness.send(Input::key(ShortcutKey::Right));
    settle(&mut harness);
    assert_eq!(player.starts(), 2, "the next recording has its own player");
    assert_eq!(
        player.alive().len(),
        1,
        "the first is let go: a recording left behind must not play on"
    );
    // A text file after it starts none.
    harness.send(Input::key(ShortcutKey::Right));
    settle(&mut harness);
    assert_eq!(player.starts(), 2);
    assert!(player.alive().is_empty(), "nothing plays over a text");
    harness.send(Input::key(ShortcutKey::Left));
    settle(&mut harness);
    assert_eq!(player.starts(), 3, "back to the song: a new player");
    harness.send(Input::key(ShortcutKey::Left));
    settle(&mut harness);
    assert_eq!(player.starts(), 4, "and back to the first: another");
    assert_eq!(player.alive().len(), 1, "only the one on screen plays");
}

#[test]
fn a_player_that_cannot_start_is_an_open_that_failed() {
    let (_dir, paths) = folder(FILES);
    let player = FakePlayer::answering(Answer::Refuses);
    let (harness, _, _) = wired(
        &paths,
        0,
        Appearance::default(),
        Wiring {
            player: Some(Arc::clone(&player)),
            ..Wiring::default()
        },
    );
    let text = harness.text_of(".viewer").unwrap_or_default();
    assert!(text.contains("can\u{2019}t be shown here"), "{text}");
    assert!(player.alive().is_empty());
}

/// A player that fails after the window opened says so in the status: (row, a position it had
/// reached before it failed, what it reports, the words the status then shows, exact or contained).
#[test]
fn a_player_that_fails_says_so_in_its_own_words() {
    let cases: [(&str, Option<u64>, MediaError, &str, bool); 2] = [
        (
            "gives up on opening",
            None,
            MediaError::OpenFailed,
            "cannot be played",
            false,
        ),
        (
            "stops mid recording",
            Some(10),
            MediaError::PlaybackFailed,
            "The player stopped",
            true,
        ),
    ];
    for (row, reached, error, words, exact) in cases {
        let Opened {
            mut harness,
            player,
            ..
        } = open();
        let line = player.latest().unwrap();
        if let Some(secs) = reached {
            playing(&line, secs);
        }
        line.say(&[MediaNotice::Failed(error)]);
        settle(&mut harness);
        let status = harness.text_of(".viewer-media-status").unwrap_or_default();
        assert!(
            if exact {
                status == words
            } else {
                status.contains(words)
            },
            "row {row}: the status {status:?} lacks {words:?}"
        );
    }
}

#[test]
fn the_panel_lists_the_tracks_and_the_chapters_and_each_row_tells_the_player() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 30);
    line.say(&[
        MediaNotice::Tracks(vec![
            track(StreamKind::Audio, 1, "Main", TrackPlay::Playing),
            track(StreamKind::Audio, 2, "Commentary", TrackPlay::Idle),
            track(StreamKind::Subtitles, 1, "English", TrackPlay::Idle),
        ]),
        MediaNotice::Chapters(vec![
            MediaChapter {
                title: "Start".to_owned(),
                start: MediaTime::from_secs(0),
            },
            MediaChapter {
                title: "Middle".to_owned(),
                start: MediaTime::from_secs(20),
            },
            MediaChapter {
                title: String::new(),
                start: MediaTime::from_secs(70),
            },
        ]),
    ]);
    settle(&mut harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    let tab = |harness: &Harness, at: usize| {
        harness
            .centre(&format!(".ds-segmented-segment:nth-child({at})"))
            .unwrap()
    };
    // The tabs are in the panel's own order: Contents, Info, Tracks.
    harness.send(Input::click(tab(&harness, 3)));
    settle(&mut harness);
    let tracks = harness.text_of(".viewer-tracks").unwrap_or_default();
    for want in [
        "Main",
        "Commentary",
        "English",
        "Off",
        "Audio",
        "Subtitles",
        "Speed",
    ] {
        assert!(tracks.contains(want), "{want} in {tracks}");
    }
    harness.send(Input::click(
        harness
            .centre(".viewer-tracks .ds-row:nth-child(5)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert!(
        line.sent().iter().any(|command| matches!(
            command,
            PlayerCommand::SelectTrack {
                kind: TrackKind::Audio,
                choice: TrackChoice::Track(TrackId(2)),
            }
        )),
        "the second audio row picks that track: {:?}",
        line.sent()
    );

    harness.send(Input::click(tab(&harness, 1)));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-chapters .ds-row"), 3);
    let chapters = harness.text_of(".viewer-chapters").unwrap_or_default();
    assert!(
        chapters.contains("Middle") && chapters.contains("0:20"),
        "{chapters}"
    );
    assert!(
        chapters.contains("Chapter 3"),
        "a chapter with no title is numbered: {chapters}"
    );
    harness.send(Input::click(
        harness
            .centre(".viewer-chapters .ds-row:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert!(
        line.sent()
            .contains(&PlayerCommand::GoToChapter(ChapterIndex(1)))
    );
}

#[test]
fn the_speed_chapter_track_and_frame_keys_reach_the_player() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 30);
    settle(&mut harness);
    for key in ['[', ']', 'n', 'p', 'a', 's', '.', ','] {
        harness.send(Input::key(ShortcutKey::Char(key)));
        settle(&mut harness);
    }
    harness.send(Input::key(ShortcutKey::Backspace));
    settle(&mut harness);
    let sent = line.sent();
    let want = [
        PlayerCommand::StepSpeed(StepDirection::Backward),
        PlayerCommand::StepSpeed(StepDirection::Forward),
        PlayerCommand::StepChapter(StepDirection::Forward),
        PlayerCommand::StepChapter(StepDirection::Backward),
        PlayerCommand::CycleTrack(TrackKind::Audio),
        PlayerCommand::CycleTrack(TrackKind::Subtitles),
        PlayerCommand::SetPlayback(Pace::Paused),
        PlayerCommand::FrameStep(StepDirection::Forward),
        PlayerCommand::FrameStep(StepDirection::Backward),
        PlayerCommand::SetSpeed(Speed::NORMAL),
    ];
    for command in want {
        assert!(sent.contains(&command), "{command:?} in {sent:?}");
    }
}

#[test]
fn the_palette_lists_the_recordings_commands() {
    let Opened { mut harness, .. } = open();
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(&mut harness);
    let text = harness.text_of(".ds-palette").unwrap_or_default();
    for want in [
        "Speed up",
        "Slow down",
        "Normal speed",
        "Next chapter",
        "Next audio track",
        "Next subtitles",
        "Mark trim start",
    ] {
        assert!(text.contains(want), "{want} in {text}");
    }
}

/// What a player that does only the basics says of itself.
fn basics_only() -> MediaNotice {
    use anyview_ui::{ControlOffer, MediaAbilities};
    MediaNotice::Abilities(MediaAbilities {
        speed: ControlOffer::Withheld,
        tracks: ControlOffer::Withheld,
        chapters: ControlOffer::Withheld,
        frame_step: ControlOffer::Withheld,
    })
}

#[test]
fn a_player_that_cannot_change_speed_or_choose_tracks_is_offered_none_of_it_and_one_that_can_still_is()
 {
    // The scripted player says nothing of its abilities, as mpv does: everything is offered.
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 25);
    hover(&mut harness);
    assert!(
        readouts(&harness).contains("1×"),
        "the speed: {}",
        readouts(&harness)
    );
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    assert_eq!(harness.count(".ds-segmented-segment"), 3, "all three tabs");

    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 25);
    line.say(&[basics_only()]);
    settle(&mut harness);
    hover(&mut harness);
    let text = readouts(&harness);
    assert!(
        text.contains("0:25") && text.contains("1:40"),
        "still a player: {text}"
    );
    assert!(!text.contains('×'), "no speed readout: {text}");
    let html = harness.html();
    assert!(
        !html.contains("Faster") && !html.contains("Slower"),
        "no speed buttons in the capsule"
    );
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(&mut harness);
    let palette = harness.text_of(".ds-palette").unwrap_or_default();
    for gone in [
        "Speed up",
        "Slow down",
        "Normal speed",
        "Next chapter",
        "Previous chapter",
        "Next audio track",
        "Next subtitles",
        "Next frame",
        "Previous frame",
    ] {
        assert!(
            !palette.to_lowercase().contains(gone),
            "{gone} in {palette}"
        );
    }
    assert!(
        palette.contains("Mark trim start"),
        "what it can do stays: {palette}"
    );
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    assert_eq!(
        harness.count(".ds-segmented-segment"),
        0,
        "only the facts: no tabs"
    );
}

#[test]
fn the_keys_of_what_a_player_cannot_do_send_it_nothing() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 30);
    line.say(&[basics_only()]);
    settle(&mut harness);
    for key in ['[', ']', 'n', 'p', 'a', 's', '.', ','] {
        harness.send(Input::key(ShortcutKey::Char(key)));
        settle(&mut harness);
    }
    harness.send(Input::key(ShortcutKey::Backspace));
    settle(&mut harness);
    let sent = line.sent();
    let unwanted = sent.iter().filter(|command| {
        matches!(
            command,
            PlayerCommand::StepSpeed(_)
                | PlayerCommand::SetSpeed(_)
                | PlayerCommand::StepChapter(_)
                | PlayerCommand::CycleTrack(_)
                | PlayerCommand::FrameStep(_)
        )
    });
    assert_eq!(unwanted.count(), 0, "{sent:?}");
}

#[test]
fn marking_a_start_and_an_end_cuts_the_export_there() {
    let Opened {
        mut harness,
        player,
        requests,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 10);
    settle(&mut harness);
    harness.send(Input::key(ShortcutKey::Char('i')));
    settle(&mut harness);
    line.say(&[MediaNotice::Position(MediaTime::from_secs(25))]);
    settle(&mut harness);
    harness.send(Input::key(ShortcutKey::Char('o')));
    settle(&mut harness);

    // Export is a palette command.
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(&mut harness);
    for letter in "export".chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(&mut harness);
    harness.send(Input::key(ShortcutKey::Enter));
    settle(&mut harness);
    // The sheet's format list has the media kinds: pick the sixth, which is the trim. Marks are
    // set, so the sheet offers the choice between the whole recording and the marks.
    harness.send(Input::click(
        harness
            .centre(".viewer-sheet .ds-list-item:nth-child(6)")
            .unwrap(),
    ));
    settle(&mut harness);
    let said = harness.text_of(".viewer-sheet").unwrap_or_default();
    assert!(
        said.contains("Range") && said.contains("Trim marks"),
        "{said}"
    );
    harness.send(Input::click(
        harness
            .centre(".viewer-sheet-buttons .ds-button:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    let exported: Vec<ExportDraft> = requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let HostRequest::Export(draft) = request {
                Some(*draft)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        exported,
        vec![ExportDraft::Media(MediaExport::Trim(
            TimeRange::new(MediaTime::from_secs(10), Some(MediaTime::from_secs(25))).unwrap()
        ))],
        "the trim keeps what the marks say"
    );
}

#[test]
fn a_cover_or_a_video_is_the_players_picture_and_a_file_with_none_is_a_card() {
    let Opened {
        mut harness,
        player,
        ..
    } = open_with(1, |_| Wiring::default());
    let line = player.latest().unwrap();
    assert_eq!(line.file.as_path().extension().unwrap(), "flac");
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH }),
        MediaNotice::Picture(VideoPresence::Absent),
    ]);
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-media-sleeve"),
        1,
        "no picture: the title"
    );
    assert_eq!(
        harness.count(".viewer-media-tile"),
        1,
        "an audio file with no cover: the music tile"
    );
    assert_eq!(
        harness.count(".viewer-media-art img, img.viewer-media-art"),
        0
    );
    assert!(
        harness
            .text_of(".viewer-media-title")
            .is_some_and(|title| title == "2-tone.flac"),
        "titled by the file's name when it has no tag"
    );

    line.say(&[MediaNotice::Picture(VideoPresence::CoverArt)]);
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-media-sleeve"),
        0,
        "a cover is the picture"
    );
    assert_eq!(harness.count(".ds-texture-layer"), 1);
}

#[test]
fn an_audio_file_with_a_cover_shows_it_and_a_window_of_its_own_size() {
    let (_dir, paths) = folder(FILES);
    let cover = ds::components::content::image_source::ImageSource::png(b"cover");
    let player = FakePlayer::answering(Answer::Plays).covered(cover.clone());
    let (mut harness, _, _) = wired(
        &paths,
        1,
        Appearance::default(),
        Wiring {
            player: Some(Arc::clone(&player)),
            ..Wiring::default()
        },
    );
    let line = player.latest().unwrap();
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH }),
        MediaNotice::Picture(VideoPresence::Absent),
    ]);
    settle(&mut harness);
    assert_eq!(harness.count("img.viewer-media-art"), 1, "the cover");
    assert_eq!(
        harness.count(".viewer-media-tile"),
        0,
        "no tile beside a cover"
    );
    assert!(harness.html().contains(&cover.0), "the cover is the image");
}

#[test]
fn a_tagged_file_is_titled_by_its_tags_in_its_own_accent() {
    let (_dir, paths) = folder(FILES);
    let player = FakePlayer::answering(Answer::Plays).tagged(MediaTags {
        title: Some("Blue Train".to_owned()),
        artist: Some("John Coltrane".to_owned()),
        album: Some("Blue Train".to_owned()),
    });
    let (mut harness, _, _) = wired(
        &paths,
        1,
        Appearance::default(),
        Wiring {
            player: Some(Arc::clone(&player)),
            ..Wiring::default()
        },
    );
    let line = player.latest().unwrap();
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded { length: LENGTH }),
        MediaNotice::Position(MediaTime::from_secs(42)),
        MediaNotice::Picture(VideoPresence::Absent),
    ]);
    hover(&mut harness);
    assert_eq!(
        harness.text_of(".viewer-media-title").as_deref(),
        Some("Blue Train")
    );
    assert_eq!(
        harness.text_of(".viewer-media-byline").as_deref(),
        Some("John Coltrane \u{b7} Blue Train")
    );
    assert_eq!(
        harness.attr(".viewer-media", "data-accent"),
        None,
        "no colour of its own"
    );
    if let Some(path) = shot("media-audio-card.png") {
        harness.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn the_mini_window_has_no_titlebar_and_keeps_the_capsule() {
    let (_dir, paths) = folder(FILES);
    let player = FakePlayer::answering(Answer::Plays);
    let (mut harness, _, _) = wired(
        &paths,
        0,
        Appearance::default(),
        Wiring {
            player: Some(Arc::clone(&player)),
            presentation: Presentation::Mini,
            ..Wiring::default()
        },
    );
    let line = player.latest().unwrap();
    playing(&line, 5);
    hover(&mut harness);
    assert_eq!(
        harness.count(".viewer-titlebar"),
        0,
        "a borderless window draws no frame"
    );
    assert_eq!(harness.count(".ds-capsule"), 1);
}

#[test]
fn what_a_recording_looks_like_is_saved_for_a_person_to_see() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 25);
    line.say(&[MediaNotice::Chapters(vec![MediaChapter {
        title: "Start".to_owned(),
        start: MediaTime::from_secs(0),
    }])]);
    hover(&mut harness);
    if let Some(path) = shot("media-video.png") {
        harness.render().unwrap().save(path).unwrap();
    }
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    if let Some(path) = shot("media-video-panel.png") {
        harness.render().unwrap().save(path).unwrap();
    }
}

/// Opens the first file with a host that answers as `answer` and offers `offer`.
fn open_answering(answer: Answer, offer: Option<MediaOffer>) -> Opened {
    let (dir, paths) = folder(FILES);
    let player = FakePlayer::answering(answer);
    let player = match offer {
        Some(offer) => player.offering(offer),
        None => player,
    };
    let memory = Arc::new(Memory::default());
    let (harness, requests, _) = wired(
        &paths,
        0,
        Appearance::default(),
        Wiring {
            player: Some(Arc::clone(&player)),
            memory: Some(Arc::clone(&memory)),
            ..Wiring::default()
        },
    );
    Opened {
        _dir: dir,
        paths,
        harness,
        requests,
        player,
        memory,
    }
}

fn needs(package: &str, purpose: &str) -> Fact {
    Fact::new(
        FactLabel::Needs,
        FactValue::text(format!("{package} (to {purpose})")),
    )
}

/// The export command from the palette.
fn export_from_the_palette(harness: &mut Harness) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    for letter in "export".chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(harness);
    harness.send(Input::key(ShortcutKey::Enter));
    settle(harness);
}

#[test]
fn a_recording_no_plugin_plays_opens_as_its_facts_and_names_the_package_that_would() {
    let Opened {
        harness, player, ..
    } = open_answering(Answer::Missing, None);
    assert_eq!(player.starts(), 0, "no player was started");
    let text = harness.text_of(".viewer").unwrap_or_default();
    assert!(
        text.contains("Needs") && text.contains("anyview-mpv (to play it)"),
        "the row that names the package: {text}"
    );
    assert!(
        text.contains("h264"),
        "and what the host read of the file: {text}"
    );
    assert_eq!(harness.count(".viewer-peek"), 1, "a facts card");
    assert_eq!(
        harness.count(".viewer-media-status"),
        0,
        "it is not an opening that never ends"
    );
    assert_eq!(
        harness.count(".ds-scrubber"),
        0,
        "there is nothing to scrub"
    );
}

#[test]
fn the_export_sheet_lists_only_the_formats_on_offer() {
    let offer = MediaOffer::new(
        vec![MediaExportKind::ToMp3, MediaExportKind::ToWav],
        Some(needs("anyview-ffmpeg", "convert it")),
    );
    let Opened {
        mut harness,
        requests,
        ..
    } = open_answering(Answer::Plays, Some(offer));
    export_from_the_palette(&mut harness);
    assert_eq!(
        harness.count(".viewer-sheet .ds-list-item"),
        2,
        "only what the machine can write"
    );
    let text = harness.text_of(".viewer-sheet").unwrap_or_default();
    assert!(
        text.contains("anyview-ffmpeg"),
        "what is missing is said: {text}"
    );
    harness.send(Input::click(
        harness
            .centre(".viewer-sheet-buttons .ds-button:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    let exported: Vec<ExportDraft> = requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let HostRequest::Export(draft) = request {
                Some(*draft)
            } else {
                None
            }
        })
        .collect();
    assert!(
        matches!(
            exported.as_slice(),
            [ExportDraft::Media(MediaExport::AudioOnly(
                anyview_core::AudioTarget::Mp3(_)
            ))]
        ),
        "the sheet opened on the first kind on offer: {exported:?}"
    );
}

#[test]
fn with_nothing_on_offer_the_sheet_says_which_package_adds_the_exports_and_writes_nothing() {
    let offer = MediaOffer::new(vec![], Some(needs("anyview-ffmpeg", "convert it")));
    let Opened {
        mut harness,
        requests,
        ..
    } = open_answering(Answer::Plays, Some(offer));
    export_from_the_palette(&mut harness);
    assert_eq!(
        harness.count(".viewer-sheet .ds-segmented-segment"),
        0,
        "no format to pick"
    );
    let text = harness.text_of(".viewer-sheet").unwrap_or_default();
    assert!(
        text.contains("anyview-ffmpeg (to convert it)"),
        "the package: {text}"
    );
    harness.send(Input::click(
        harness.centre(".viewer-sheet-buttons .ds-button").unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-sheet"), 0, "OK puts the sheet away");
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| !matches!(request, HostRequest::Export(_))),
        "nothing was asked of the host"
    );
}

/// A recording in a window `width` wide, playing, with the pointer on it so the capsule shows.
fn narrow(width: u32) -> (Harness, tempfile::TempDir) {
    let Opened {
        mut harness,
        player,
        _dir,
        ..
    } = open_with(0, |_| Wiring {
        viewport: Some(Viewport {
            width,
            height: 320,
            scale_percent: 100,
        }),
        ..Wiring::default()
    });
    playing(&player.latest().unwrap(), 25);
    harness.send(Input::pointer_move(Point {
        x: Px(width as f32 / 2.0),
        y: Px(160.0),
    }));
    settle(&mut harness);
    (harness, _dir)
}

#[test]
fn a_capsule_in_a_narrow_window_keeps_play_and_the_bar_and_fits_the_stage() {
    let (harness, _dir) = narrow(480);
    let capsule = harness.rect(".ds-capsule").expect("the capsule shows");
    assert!(
        capsule.origin.x.0 >= 0.0 && capsule.origin.x.0 + capsule.size.width.0 <= 480.0,
        "the capsule is inside the window: {capsule:?}"
    );
    assert_eq!(
        harness.count(".ds-capsule .ds-button"),
        3,
        "back, play, forward"
    );
    assert_eq!(harness.count(".ds-scrubber"), 1, "the bar stays");
    assert_eq!(
        harness.count(".ds-capsule .ds-slider"),
        1,
        "the level stays: the 120 bar leaves it room"
    );
    let text = readouts(&harness);
    assert!(text.contains("0:25"), "the clock stays: {text}");
    assert!(
        !text.contains("1:40") && !text.contains("1×"),
        "the length and the speed went: {text}"
    );
    let bar = harness.rect(".ds-capsule-scrub").expect("the bar");
    assert!(
        bar.size.width.0 >= 120.0,
        "the bar is no narrower than its least: {bar:?}"
    );
}

#[test]
fn a_capsule_in_a_wide_window_shows_all_it_has_room_for() {
    let (harness, _dir) = narrow(900);
    let capsule = harness.rect(".ds-capsule").expect("the capsule shows");
    assert!(
        capsule.origin.x.0 >= 0.0 && capsule.origin.x.0 + capsule.size.width.0 <= 900.0,
        "{capsule:?}"
    );
    assert_eq!(
        harness.count(".ds-capsule .ds-button"),
        6,
        "back, play, forward, slower, faster, export: the whole 640 capsule fits a 900 stage"
    );
    assert_eq!(harness.count(".ds-scrubber"), 1);
    assert_eq!(harness.count(".ds-capsule .ds-slider"), 1, "the level");
    let text = readouts(&harness);
    for shown in ["0:25", "1:40", "1×"] {
        assert!(text.contains(shown), "{shown} in {text}");
    }
    // Everything fits inside the capsule's own box.
    let bar = harness.rect(".ds-capsule-scrub").expect("the bar");
    let level = harness.rect(".ds-capsule-level").expect("the level");
    assert!(
        level.origin.x.0 + level.size.width.0 <= capsule.origin.x.0 + capsule.size.width.0,
        "{level:?} in {capsule:?}"
    );
    assert!(bar.size.width.0 >= 120.0, "{bar:?}");
}

#[test]
fn the_capsule_thins_out_at_quires_thresholds_as_the_window_narrows() {
    // Buttons (back, play, forward, then slower, faster, export), the level, and the readouts.
    /// A width, the buttons and the level the capsule shows there, and the readouts it shows and drops.
    type Case = (
        &'static str,
        u32,
        usize,
        bool,
        &'static [&'static str],
        &'static [&'static str],
    );
    const CASES: &[Case] = &[
        (
            "everything fits",
            672,
            6,
            true,
            &["0:25", "1:40", "1×"],
            &[],
        ),
        (
            "the export goes first",
            671,
            5,
            true,
            &["0:25", "1:40", "1×"],
            &[],
        ),
        (
            "the speed still fits",
            623,
            5,
            true,
            &["0:25", "1:40", "1×"],
            &[],
        ),
        ("the speed goes", 622, 3, true, &["0:25", "1:40"], &["1×"]),
        (
            "the length still fits",
            498,
            3,
            true,
            &["0:25", "1:40"],
            &["1×"],
        ),
        ("the length goes", 497, 3, true, &["0:25"], &["1:40", "1×"]),
        ("the level still fits", 450, 3, true, &["0:25"], &["1:40"]),
        ("the level goes", 449, 3, false, &["0:25"], &["1:40"]),
        ("the clock still fits", 337, 3, false, &["0:25"], &["1:40"]),
        (
            "the clock goes",
            336,
            3,
            false,
            &[],
            &["0:25", "1:40", "1×"],
        ),
    ];
    for (name, width, buttons, level, shown, gone) in CASES {
        let (harness, _dir) = narrow(*width);
        assert_eq!(
            harness.count(".ds-capsule .ds-button"),
            *buttons,
            "{name} at {width}"
        );
        assert_eq!(
            harness.count(".ds-capsule .ds-slider") == 1,
            *level,
            "{name} at {width}"
        );
        assert_eq!(
            harness.count(".ds-scrubber"),
            1,
            "the bar stays {name} at {width}"
        );
        let text = readouts(&harness);
        for one in *shown {
            assert!(text.contains(one), "{name} at {width}: {one} in {text}");
        }
        for one in *gone {
            assert!(
                !text.contains(one),
                "{name} at {width}: {one} gone from {text}"
            );
        }
    }
}

#[test]
fn a_panel_open_on_a_tab_the_player_then_withholds_shows_the_facts_not_a_blank() {
    let Opened {
        mut harness,
        player,
        ..
    } = open();
    let line = player.latest().unwrap();
    playing(&line, 25);
    settle(&mut harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    // A player that offers everything opens the panel on its first tab, the chapters.
    assert!(harness.html().contains("viewer-chapters") || harness.html().contains("no chapters"));
    line.say(&[basics_only()]);
    settle(&mut harness);
    let panel = harness.text_of(".viewer-side-panel").unwrap_or_default();
    assert!(
        panel.contains("WebM video"),
        "the file's facts fill the panel once the tab is gone: {panel:?}"
    );
}

#[test]
fn the_dialog_offers_a_bitrate_for_lossy_audio_only_and_a_range_only_where_marks_exist() {
    let offer = MediaOffer::new(
        vec![
            MediaExportKind::ToMp3,
            MediaExportKind::ToFlac,
            MediaExportKind::Trim,
        ],
        None,
    );
    let Opened {
        mut harness,
        requests,
        ..
    } = open_answering(Answer::Plays, Some(offer));
    export_from_the_palette(&mut harness);
    let said = |harness: &Harness| harness.text_of(".viewer-sheet").unwrap_or_default();
    let click = |harness: &mut Harness, selector: &str| {
        let at = harness.centre(selector).unwrap();
        harness.send(Input::click(at));
        settle(harness);
    };
    assert!(
        said(&harness).contains("Bitrate"),
        "mp3: {}",
        said(&harness)
    );
    click(
        &mut harness,
        ".viewer-sheet [aria-label=\"Bitrate\"] .ds-segmented-segment:nth-child(4)",
    );
    click(&mut harness, ".viewer-sheet .ds-list-item:nth-child(3)");
    assert!(
        !said(&harness).contains("Bitrate"),
        "flac: {}",
        said(&harness)
    );
    click(&mut harness, ".viewer-sheet .ds-list-item:nth-child(1)");
    assert!(
        !said(&harness).contains("Range"),
        "no marks: {}",
        said(&harness)
    );
    click(&mut harness, ".viewer-sheet .ds-list-item:nth-child(2)");
    click(
        &mut harness,
        ".viewer-sheet [aria-label=\"Bitrate\"] .ds-segmented-segment:nth-child(4)",
    );
    click(
        &mut harness,
        ".viewer-sheet-buttons .ds-button:nth-child(2)",
    );
    let exported: Vec<ExportDraft> = requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let HostRequest::Export(draft) = request {
                Some(*draft)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        exported,
        [ExportDraft::Media(MediaExport::AudioOnly(
            anyview_core::AudioTarget::Mp3(anyview_core::Bitrate::from_kbps(256))
        ))]
    );
}
