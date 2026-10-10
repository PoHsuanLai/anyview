//! The capsule's controls for a recording: the seek and play buttons, the clock and the progress
//! bar, the volume, the speed and, for a video, saving the frame. Every button is a command, the
//! bar and the volume are slots the window maps to machine inputs.

use super::doc::MediaDoc;
use super::panel::{clock_text, speed_text};
use crate::families::view::StageCx;
use crate::{AfterScrub, Command, ControlOffer, MediaStage, Stage, StageCommand};
use anyview_core::{FileAction, FormatKind, MediaLength, MediaTime, Volume};
use ds::components::chrome::capsule::model::{CapsuleSlot, LevelSlot, ScrubSlot};
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::components::controls::scrubber_model::BufferedRange;
use ds::motion::spring::Millis;
use ds::prelude::Icon;
use ds_core::vocab::{Availability, Fraction};

/// Whether the recording is advancing, as the capsule's play button shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Playing,
    Held,
}

/// Where playback is, how long it runs and whether it advances; `None` while there is nothing
/// to control (opening, failed).
fn position_of(stage: &MediaStage) -> Option<(MediaTime, MediaLength, Shown)> {
    match stage {
        MediaStage::Playing { at, length } => Some((*at, *length, Shown::Playing)),
        MediaStage::Paused { at, length } | MediaStage::Ended { at, length } => {
            Some((*at, *length, Shown::Held))
        }
        MediaStage::Scrubbing {
            to, length, resume, ..
        } => Some((
            *to,
            *length,
            match resume {
                AfterScrub::Play => Shown::Playing,
                AfterScrub::Stay => Shown::Held,
            },
        )),
        MediaStage::Opening | MediaStage::Failed(_) => None,
    }
}

/// `at` of the way through `length`, in thousandths; the start for a recording of no length.
pub(crate) fn fraction_of(at: MediaTime, length: MediaLength) -> Fraction {
    match length.0.0 {
        0 => Fraction(0),
        whole => {
            let thousandths = u128::from(at.0.min(whole)) * 1000 / u128::from(whole);
            Fraction(u16::try_from(thousandths).unwrap_or(1000))
        }
    }
}

/// The time `at` thousandths of the way through `length`.
pub(crate) fn place_to_time(at: Fraction, length: MediaLength) -> MediaTime {
    let thousandths = u128::from(at.0.min(1000));
    MediaTime(u64::try_from(u128::from(length.0.0) * thousandths / 1000).unwrap_or(u64::MAX))
}

/// A level as a fraction of the loudest volume, which is a hundred and fifty percent.
pub(crate) fn level_of(volume: Volume) -> Fraction {
    let loudest = u32::from(Volume::MAX.percent().0);
    Fraction(
        u16::try_from((u32::from(volume.percent().0) * 1000 + loudest / 2) / loudest)
            .unwrap_or(1000),
    )
}

/// The volume at a place on the level slider.
pub(crate) fn level_to_volume(at: Fraction) -> Volume {
    let loudest = u32::from(Volume::MAX.percent().0);
    Volume::clamped(anyview_core::Percent(
        u16::try_from((u32::from(at.0.min(1000)) * loudest + 500) / 1000).unwrap_or(0),
    ))
}

/// How soon each control goes when the capsule is too wide for the stage: the export first (it
/// stays in the palette and the context menu, and it is the one control a stage under 672 has no
/// room for beside the speed), then the speed, the length, the level (the volume is the system's
/// still) and the clock.
const RANK_EXPORT: u8 = 5;
const RANK_SPEED: u8 = 4;
const RANK_LENGTH: u8 = 3;
const RANK_LEVEL: u8 = 2;
const RANK_CLOCK: u8 = 1;

/// The controls, left to right, as many as the stage has room for.
pub(super) fn slots(doc: &MediaDoc, cx: &StageCx) -> Vec<RankedSlot<Command>> {
    // A recording nothing plays has no controls of a player; what can still be done is the export.
    if doc.needs().is_some() {
        return vec![
            CapsuleSlot::button(Command::File(FileAction::Export), "Export", Icon::Camera)
                .essential(),
        ];
    }
    let Stage::Media(stage) = &cx.stage else {
        return Vec::new();
    };
    let Some((at, length, shown)) = position_of(stage) else {
        return Vec::new();
    };
    let live = cx.media.read();
    let stage_command = |command| Command::Stage(command);
    let (play_label, play_icon) = match shown {
        Shown::Playing => ("Pause", Icon::Pause),
        Shown::Held => ("Play", Icon::Play),
    };
    let buffered = BufferedRange {
        from: Fraction(0),
        to: Fraction(live.buffered.0.min(100) * 10),
    };
    // What goes when the stage is narrow, soonest first (quire's capsule): the export, the speed, the
    // length, the level, then the clock. Play, the seek buttons and the bar stay.
    let mut slots = vec![
        CapsuleSlot::button(
            stage_command(StageCommand::SeekBack),
            "Skip Back",
            Icon::SkipBack,
        )
        .essential(),
        CapsuleSlot::button(
            stage_command(StageCommand::TogglePlayback),
            play_label,
            play_icon,
        )
        .essential(),
        CapsuleSlot::button(
            stage_command(StageCommand::SeekForward),
            "Skip Forward",
            Icon::SkipForward,
        )
        .essential(),
        CapsuleSlot::Divider.essential(),
        CapsuleSlot::Readout(clock_text(at)).droppable(RANK_CLOCK),
        CapsuleSlot::Scrub(ScrubSlot {
            label: "Position".to_owned(),
            position: fraction_of(at, length),
            length: Millis(u32::try_from(length.0.as_millis()).unwrap_or(u32::MAX)),
            buffered: vec![buffered],
            availability: Availability::Enabled,
        })
        .essential(),
        CapsuleSlot::Readout(clock_text(length.0)).droppable(RANK_LENGTH),
        CapsuleSlot::Divider.essential(),
        CapsuleSlot::Level(LevelSlot {
            label: "Volume".to_owned(),
            value: level_of(live.volume),
            availability: Availability::Enabled,
        })
        .droppable(RANK_LEVEL),
    ];
    // What the player cannot do has no control.
    match live.abilities.speed {
        ControlOffer::Offered => slots.extend([
            CapsuleSlot::Divider.essential(),
            CapsuleSlot::button(stage_command(StageCommand::SlowDown), "Slower", Icon::Minus)
                .droppable(RANK_SPEED),
            CapsuleSlot::Readout(speed_text(live.speed)).droppable(RANK_SPEED),
            CapsuleSlot::button(stage_command(StageCommand::SpeedUp), "Faster", Icon::Plus)
                .droppable(RANK_SPEED),
        ]),
        ControlOffer::Withheld => {}
    }
    match doc.kind {
        FormatKind::Video => {
            slots.push(CapsuleSlot::Divider.essential());
            slots.push(
                CapsuleSlot::button(Command::File(FileAction::Export), "Export", Icon::Camera)
                    .droppable(RANK_EXPORT),
            );
        }
        FormatKind::Audio
        | FormatKind::Pdf
        | FormatKind::Raster
        | FormatKind::Vector
        | FormatKind::Markdown
        | FormatKind::Code
        | FormatKind::PlainText
        | FormatKind::Table
        | FormatKind::Tree
        | FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Book
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => {}
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_and_its_fraction_agree_both_ways() {
        let length = MediaLength(MediaTime::from_secs(200));
        const CASES: &[(&str, u64, u16)] = &[
            ("the start", 0, 0),
            ("a quarter", 50, 250),
            ("the middle", 100, 500),
            ("the end", 200, 1000),
            ("past the end holds at the end", 900, 1000),
        ];
        for (name, secs, want) in CASES {
            let at = MediaTime::from_secs(*secs);
            assert_eq!(fraction_of(at, length), Fraction(*want), "{name}");
        }
        assert_eq!(
            place_to_time(Fraction(250), length),
            MediaTime::from_secs(50)
        );
        assert_eq!(place_to_time(Fraction(5000), length), length.0);
        assert_eq!(
            fraction_of(MediaTime::from_secs(5), MediaLength(MediaTime(0))),
            Fraction(0)
        );
    }

    #[test]
    fn the_level_slider_runs_to_the_loudest_volume() {
        const CASES: &[(&str, u16, u16)] = &[
            ("silent", 0, 0),
            ("full is two thirds", 100, 667),
            ("the loudest", 150, 1000),
        ];
        for (name, percent, level) in CASES {
            let volume = Volume::clamped(anyview_core::Percent(*percent));
            assert_eq!(level_of(volume), Fraction(*level), "{name}");
        }
        assert_eq!(level_to_volume(Fraction(1000)), Volume::MAX);
        assert_eq!(level_to_volume(Fraction(0)), Volume::SILENT);
        assert_eq!(
            level_to_volume(Fraction(667)).percent(),
            anyview_core::Percent(100)
        );
    }
}
