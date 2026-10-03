use super::*;
use anyview_core::{MediaLength, MediaTime, Percent, TrackChoice, Volume};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const LENGTH: MediaLength = MediaLength(MediaTime::from_secs(100));

const fn secs(seconds: u64) -> MediaTime {
    MediaTime::from_secs(seconds)
}
const fn playing(seconds: u64) -> MediaStage {
    MediaStage::Playing {
        at: secs(seconds),
        length: LENGTH,
    }
}
const fn paused(seconds: u64) -> MediaStage {
    MediaStage::Paused {
        at: secs(seconds),
        length: LENGTH,
    }
}
const fn ended(seconds: u64) -> MediaStage {
    MediaStage::Ended {
        at: secs(seconds),
        length: LENGTH,
    }
}
const fn scrubbing(from: u64, to: u64, resume: AfterScrub) -> MediaStage {
    MediaStage::Scrubbing {
        from: secs(from),
        to: secs(to),
        length: LENGTH,
        resume,
    }
}
const fn event(event: PlayerEvent) -> MediaIn {
    MediaIn::Player(event)
}
const fn stopped(reason: EndReason) -> MediaIn {
    MediaIn::Player(PlayerEvent::Ended(reason))
}
const fn seek(seconds: u64) -> MediaOut {
    MediaOut::Command(PlayerCommand::Seek(secs(seconds)))
}
const fn pace(pace: Pace) -> MediaOut {
    MediaOut::Command(PlayerCommand::SetPlayback(pace))
}

/// Name, state before, input, state after, outputs. Seek steps are 5 s.
type Case = (
    &'static str,
    MediaStage,
    MediaIn,
    MediaStage,
    &'static [MediaOut],
);

const CASES: &[Case] = &[
    (
        "the file opening starts playback at zero",
        MediaStage::Opening,
        event(PlayerEvent::Loaded { length: LENGTH }),
        playing(0),
        &[],
    ),
    (
        "an error while opening fails the open",
        MediaStage::Opening,
        stopped(EndReason::Error),
        MediaStage::Failed(MediaError::OpenFailed),
        &[],
    ),
    (
        "a player that cannot start fails the stage",
        MediaStage::Opening,
        MediaIn::Failed(MediaError::OpenFailed),
        MediaStage::Failed(MediaError::OpenFailed),
        &[],
    ),
    (
        "the cache level shows while opening",
        MediaStage::Opening,
        event(PlayerEvent::Buffering(Percent(40))),
        MediaStage::Opening,
        &[MediaOut::Buffering(Percent(40))],
    ),
    (
        "a volume set while opening reaches the player",
        MediaStage::Opening,
        MediaIn::SetVolume(Volume::FULL),
        MediaStage::Opening,
        &[MediaOut::Command(PlayerCommand::SetVolume(Volume::FULL))],
    ),
    (
        "toggle before the file is open is nothing",
        MediaStage::Opening,
        MediaIn::Toggle,
        MediaStage::Opening,
        &[],
    ),
    (
        "the polled position moves the clock",
        playing(10),
        MediaIn::Position(secs(20)),
        playing(20),
        &[],
    ),
    (
        "a position past the end is clamped",
        playing(10),
        MediaIn::Position(secs(150)),
        playing(100),
        &[],
    ),
    (
        "toggle pauses",
        playing(20),
        MediaIn::Toggle,
        paused(20),
        &[pace(Pace::Paused)],
    ),
    (
        "seeking forward moves one step",
        playing(20),
        MediaIn::SeekForward,
        playing(25),
        &[seek(25)],
    ),
    (
        "seeking forward stops at the end",
        playing(98),
        MediaIn::SeekForward,
        playing(100),
        &[seek(100)],
    ),
    (
        "seeking back stops at the start",
        playing(3),
        MediaIn::SeekBack,
        playing(0),
        &[seek(0)],
    ),
    (
        "seeking to a position past the end is clamped",
        playing(3),
        MediaIn::SeekTo(secs(500)),
        playing(100),
        &[seek(100)],
    ),
    (
        "grabbing the slider while playing holds the player and remembers to resume",
        playing(20),
        MediaIn::ScrubStart,
        scrubbing(20, 20, AfterScrub::Play),
        &[pace(Pace::Paused)],
    ),
    (
        "a frame step holds first",
        playing(20),
        MediaIn::FrameStep(FrameDirection::Forward),
        paused(20),
        &[
            pace(Pace::Paused),
            MediaOut::Command(PlayerCommand::FrameStep(FrameDirection::Forward)),
        ],
    ),
    (
        "another control pausing the player is followed",
        playing(20),
        event(PlayerEvent::Playback(Pace::Paused)),
        paused(20),
        &[],
    ),
    (
        "a playing report while playing is nothing",
        playing(20),
        event(PlayerEvent::Playback(Pace::Playing)),
        playing(20),
        &[],
    ),
    (
        "reaching the end ends at the length",
        playing(99),
        stopped(EndReason::Eof),
        ended(100),
        &[],
    ),
    (
        "a stop ends where it was",
        playing(20),
        stopped(EndReason::Stop),
        ended(20),
        &[],
    ),
    (
        "the player erroring fails playback",
        playing(20),
        stopped(EndReason::Error),
        MediaStage::Failed(MediaError::PlaybackFailed),
        &[],
    ),
    (
        "the player quitting is not the end of the file",
        playing(20),
        stopped(EndReason::Quit),
        playing(20),
        &[],
    ),
    (
        "a seek landing changes nothing",
        playing(20),
        event(PlayerEvent::SeekDone),
        playing(20),
        &[],
    ),
    (
        "a volume change is shown",
        playing(20),
        event(PlayerEvent::VolumeChanged(Volume::SILENT)),
        playing(20),
        &[MediaOut::VolumeChanged(Volume::SILENT)],
    ),
    (
        "a track change asks for the list again",
        playing(20),
        event(PlayerEvent::TracksChanged),
        playing(20),
        &[MediaOut::TracksChanged],
    ),
    (
        "choosing a track tells the player",
        playing(20),
        MediaIn::Select {
            kind: TrackKind::Subtitles,
            choice: TrackChoice::Off,
        },
        playing(20),
        &[MediaOut::Command(PlayerCommand::SelectTrack {
            kind: TrackKind::Subtitles,
            choice: TrackChoice::Off,
        })],
    ),
    (
        "toggle plays",
        paused(20),
        MediaIn::Toggle,
        playing(20),
        &[pace(Pace::Playing)],
    ),
    (
        "seeking while held stays held",
        paused(20),
        MediaIn::SeekForward,
        paused(25),
        &[seek(25)],
    ),
    (
        "grabbing the slider while held stays held after",
        paused(20),
        MediaIn::ScrubStart,
        scrubbing(20, 20, AfterScrub::Stay),
        &[],
    ),
    (
        "a frame step while held steps",
        paused(20),
        MediaIn::FrameStep(FrameDirection::Backward),
        paused(20),
        &[MediaOut::Command(PlayerCommand::FrameStep(
            FrameDirection::Backward,
        ))],
    ),
    (
        "another control playing the player is followed",
        paused(20),
        event(PlayerEvent::Playback(Pace::Playing)),
        playing(20),
        &[],
    ),
    (
        "reaching the end while held ends too",
        paused(99),
        stopped(EndReason::Eof),
        ended(100),
        &[],
    ),
    (
        "the slider moving seeks live",
        scrubbing(20, 20, AfterScrub::Play),
        MediaIn::ScrubTo(secs(60)),
        scrubbing(20, 60, AfterScrub::Play),
        &[seek(60)],
    ),
    (
        "the slider past the end is clamped",
        scrubbing(20, 20, AfterScrub::Play),
        MediaIn::ScrubTo(secs(500)),
        scrubbing(20, 100, AfterScrub::Play),
        &[seek(100)],
    ),
    (
        "letting go resumes if it was playing",
        scrubbing(20, 60, AfterScrub::Play),
        MediaIn::ScrubEnd,
        playing(60),
        &[seek(60), pace(Pace::Playing)],
    ),
    (
        "letting go stays held if it was held",
        scrubbing(20, 60, AfterScrub::Stay),
        MediaIn::ScrubEnd,
        paused(60),
        &[seek(60)],
    ),
    (
        "cancelling goes back to where the scrub began",
        scrubbing(20, 60, AfterScrub::Play),
        MediaIn::ScrubCancel,
        playing(20),
        &[seek(20), pace(Pace::Playing)],
    ),
    (
        "the position poll does not fight the slider",
        scrubbing(20, 60, AfterScrub::Play),
        MediaIn::Position(secs(21)),
        scrubbing(20, 60, AfterScrub::Play),
        &[],
    ),
    (
        "the player echoing our pause is nothing",
        scrubbing(20, 60, AfterScrub::Play),
        event(PlayerEvent::Playback(Pace::Paused)),
        scrubbing(20, 60, AfterScrub::Play),
        &[],
    ),
    (
        "toggle during a scrub is nothing",
        scrubbing(20, 60, AfterScrub::Play),
        MediaIn::Toggle,
        scrubbing(20, 60, AfterScrub::Play),
        &[],
    ),
    (
        "toggle at the end plays again from the start",
        ended(100),
        MediaIn::Toggle,
        playing(0),
        &[seek(0), pace(Pace::Playing)],
    ),
    (
        "seeking back from the end holds there",
        ended(100),
        MediaIn::SeekBack,
        paused(95),
        &[seek(95), pace(Pace::Paused)],
    ),
    (
        "grabbing the slider at the end stays held after",
        ended(100),
        MediaIn::ScrubStart,
        scrubbing(100, 100, AfterScrub::Stay),
        &[],
    ),
    (
        "a failed stage ignores toggle",
        MediaStage::Failed(MediaError::PlaybackFailed),
        MediaIn::Toggle,
        MediaStage::Failed(MediaError::PlaybackFailed),
        &[],
    ),
    (
        "a failed stage ignores the position",
        MediaStage::Failed(MediaError::OpenFailed),
        MediaIn::Position(secs(1)),
        MediaStage::Failed(MediaError::OpenFailed),
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    let params = MediaParams::default();
    for (name, from, input, state, outs) in CASES {
        let (next, out) = from.step(*input, Stamp(0), &params);
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn a_scrub_restores_what_the_person_had() {
    // Dragging while playing resumes; dragging while held does not.
    let params = MediaParams::default();
    for (name, start, resumes) in [
        ("playing", playing(10), true),
        ("paused", paused(10), false),
    ] {
        let (grabbed, _) = start.step(MediaIn::ScrubStart, Stamp(0), &params);
        let (moved, _) = grabbed.step(MediaIn::ScrubTo(secs(40)), Stamp(1), &params);
        let (released, outs) = moved.step(MediaIn::ScrubEnd, Stamp(2), &params);
        let resume = outs.contains(&pace(Pace::Playing));
        assert_eq!(resume, resumes, "{name}: resumed");
        assert_eq!(
            matches!(released, MediaStage::Playing { .. }),
            resumes,
            "{name}: state"
        );
    }
}
