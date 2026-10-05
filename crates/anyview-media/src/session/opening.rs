//! The session between asking for a file and mpv saying it is open.

use super::report::{Frame, Report};
use super::{Idle, Loaded, Opening, Session, convert, fault};
use crate::event::{EndReason, MediaEvent};
use mpv_wgpu_player as mpv;

/// What a poll of an opening session found: it is still opening, it opened, or it gave up.
#[derive(Debug)]
pub enum Opened {
    /// mpv has not said yet.
    Waiting(Session<Opening>, Report),
    /// The file is open.
    Loaded(Session<Loaded>, Report),
    /// The file would not open; the player is idle again, and the report names why.
    Failed(Session<Idle>, Report),
}

impl Session<Opening> {
    /// Drain mpv and draw. The report's events are in the order mpv gave them.
    pub fn poll(mut self) -> Opened {
        let report = match drain(&mut self.player) {
            Ok(report) => report,
            Err(reason) => return self.give_up(failed(reason)),
        };
        let loaded = report
            .events
            .iter()
            .any(|event| matches!(event, MediaEvent::Loaded { .. }));
        let errored = report.events.contains(&MediaEvent::Ended(EndReason::Error));
        match (loaded, errored) {
            (_, true) => self.give_up(report),
            (true, false) => Opened::Loaded(self.with_state(Loaded), report),
            (false, false) => Opened::Waiting(self, report),
        }
    }

    fn give_up(self, mut report: Report) -> Opened {
        if !report
            .events
            .iter()
            .any(|event| matches!(event, MediaEvent::Failed(_)))
        {
            report
                .events
                .push(MediaEvent::Failed("the file would not open".to_owned()));
        }
        Opened::Failed(self.with_state(Idle), report)
    }
}

fn failed(reason: String) -> Report {
    Report {
        events: vec![MediaEvent::Failed(reason)],
        frame: Frame::Unchanged,
    }
}

/// Poll the player and translate what it said.
pub(super) fn drain(player: &mut mpv::Player) -> Result<Report, String> {
    let outcome = player.poll().map_err(|error| fault(error).to_string())?;
    let events: Vec<mpv::Event> = player.events().to_vec();
    let mut translated = Vec::new();
    for event in &events {
        translate(player, event, &mut translated);
    }
    let frame = match outcome.presentation {
        mpv::Presentation::Updated => Frame::Rewritten,
        mpv::Presentation::Unchanged => Frame::Unchanged,
    };
    Ok(Report {
        events: translated,
        frame,
    })
}

fn translate(player: &mpv::Player, event: &mpv::Event, out: &mut Vec<MediaEvent>) {
    match event {
        mpv::Event::Loaded => out.push(MediaEvent::Loaded {
            length: player.duration().map(convert::length),
        }),
        mpv::Event::Ended(reason) => out.push(MediaEvent::Ended(convert::end_reason(*reason))),
        mpv::Event::Playback(playback) => out.push(MediaEvent::Playback(convert::pace(*playback))),
        mpv::Event::SeekDone => out.push(MediaEvent::SeekDone),
        mpv::Event::Buffering(percent) => {
            out.push(MediaEvent::Buffering(convert::percent(*percent)))
        }
        mpv::Event::TracksChanged => {
            let tracks = player.tracks();
            out.push(MediaEvent::Tracks(
                tracks.iter().map(convert::track_of).collect(),
            ));
            out.push(MediaEvent::Picture(convert::presence(player.has_video())));
        }
        mpv::Event::ChaptersChanged => out.push(MediaEvent::Chapters(
            player.chapters().iter().map(convert::chapter_of).collect(),
        )),
        mpv::Event::VolumeChanged => {
            out.push(MediaEvent::Volume(convert::volume_of(player.volume())));
        }
    }
}
