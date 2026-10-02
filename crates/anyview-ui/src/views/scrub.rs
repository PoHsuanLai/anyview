//! What the capsule's progress bar and level did, as inputs for the media stage. The bar speaks
//! in thousandths of the length; the stage in times, so the length is read from the stage.

use super::shelf::Dispatch;
use crate::families::{level_to_volume, place_to_time};
use crate::{MediaIn, Stage, StageIn, ViewerIn};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::ScrubEvent;
use ds_core::vocab::Fraction;

fn send(dispatch: Dispatch, input: MediaIn) {
    dispatch.send(ViewerIn::Stage(StageIn::Media(input)));
}

/// The progress bar did `event`: a press grabs it and moves to the place, a drag moves, a
/// release lets go there, and a key seeks.
pub(super) fn scrubbed(dispatch: Dispatch, event: ScrubEvent) {
    let state = dispatch.machine.state().peek().clone();
    let Stage::Media(stage) = &state.stage else {
        return;
    };
    let Some(length) = stage.length() else {
        return;
    };
    match event {
        ScrubEvent::Start(at) => {
            send(dispatch, MediaIn::ScrubStart);
            send(dispatch, MediaIn::ScrubTo(place_to_time(at, length)));
        }
        ScrubEvent::Move(at) => send(dispatch, MediaIn::ScrubTo(place_to_time(at, length))),
        ScrubEvent::End(at) => {
            send(dispatch, MediaIn::ScrubTo(place_to_time(at, length)));
            send(dispatch, MediaIn::ScrubEnd);
        }
        ScrubEvent::Cancel => send(dispatch, MediaIn::ScrubCancel),
        ScrubEvent::Seek(at) => send(dispatch, MediaIn::SeekTo(place_to_time(at, length))),
    }
}

/// The level slider moved to `at`.
pub(super) fn levelled(dispatch: Dispatch, at: Fraction) {
    send(dispatch, MediaIn::SetVolume(level_to_volume(at)));
}
