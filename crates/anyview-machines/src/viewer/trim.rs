//! The viewer's side of trimming a recording: Trim… itself, the bar's inputs stepped, the player
//! held inside the stretch that plays, and saving the trim over the file.

use super::model::{Viewer, ViewerOut, ViewerParams};
use super::region::{Step, sheet, unbounded};
use crate::context::ContextMenu;
use crate::edits::{EditOffer, EditRequest};
use crate::palette::Palette;
use crate::sheet::{Sheet, SheetIn};
use crate::stage::{MediaIn, MediaStage, Stage, StageIn, TrimEdge};
use crate::trim::{MediaTrim, TrimIn, TrimWindow};
use anyview_core::{MediaExportKind, MediaTime};
use ds_core::time::stamp::Stamp;
use ds_core::vocab::ShortcutKey;

/// Trim… (⌘T): the bar comes up over the recording, which is held while it is chosen from. A
/// recording nothing here can write says which package adds the cut, as an export does.
pub(super) fn begin(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let length = match &viewer.stage {
        Stage::Media(media) => media.length(),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    };
    let Some(length) = length else {
        return (viewer, Vec::new());
    };
    let offer = &params.sheet.media;
    if offer.offers(MediaExportKind::Trim) {
        return trim_in(viewer, TrimIn::Begin(length), at, params);
    }
    match offer.needs() {
        Some(needs) => sheet(
            viewer,
            SheetIn::OpenUnavailable(needs.clone(), offer.helper()),
            at,
            params,
        ),
        None => (viewer, Vec::new()),
    }
}

/// An input of the trim. The player follows what it changes: held while the bar comes up, at the
/// handle that moved while one is dragged, and at the start of what is kept once the bar goes.
pub(super) fn trim_in(viewer: Viewer, input: TrimIn, at: Stamp, params: &ViewerParams) -> Step {
    let Stage::Media(media) = &viewer.stage else {
        return (viewer, Vec::new());
    };
    let trim = viewer.trim.clone().step(input);
    if trim == viewer.trim {
        return (viewer, Vec::new());
    }
    let follows = follows(media, &trim, input);
    let mut state = (Viewer { trim, ..viewer }, Vec::new());
    for next in follows {
        let (viewer, mut outs) = state;
        let (viewer, more) = super::region::stage(viewer, StageIn::Media(next), at, params);
        outs.extend(more);
        state = (viewer, outs);
    }
    state
}

/// What the player is told after `input` changed the trim to `trim`.
fn follows(media: &MediaStage, trim: &MediaTrim, input: TrimIn) -> Vec<MediaIn> {
    let hold = matches!(media, MediaStage::Playing { .. }).then_some(MediaIn::Toggle);
    let to_start = |window: Option<TrimWindow>| {
        MediaIn::SeekTo(window.map_or_else(MediaTime::default, |window| window.start))
    };
    match input {
        TrimIn::Begin(_) => hold.into_iter().collect(),
        TrimIn::Move { edge, .. } => trim
            .handles()
            .map(|(_, window)| {
                MediaIn::SeekTo(match edge {
                    TrimEdge::Start => window.start,
                    TrimEdge::End => window.end,
                })
            })
            .into_iter()
            .collect(),
        TrimIn::Apply | TrimIn::Undo | TrimIn::Redo => hold
            .into_iter()
            .chain([to_start(trim.play_range())])
            .collect(),
        TrimIn::Cancel => hold
            .into_iter()
            .chain(trim.applied().map(|window| to_start(Some(window))))
            .collect(),
        TrimIn::Discard => Vec::new(),
    }
}

/// An input of the media stage while a stretch plays: the player is kept inside it. A seek or a
/// scrub lands at its nearest end, play from outside it (or from its end) starts at its start,
/// and playing past its end holds the player there.
pub(super) fn within(
    viewer: Viewer,
    window: TrimWindow,
    input: MediaIn,
    at: Stamp,
    params: &ViewerParams,
) -> Step {
    let Stage::Media(media) = &viewer.stage else {
        return unbounded(viewer, StageIn::Media(input), at, params);
    };
    let inputs = held_to(media, window, input, params.stage.media.seek_step);
    let mut state = (viewer, Vec::new());
    for next in inputs {
        let (viewer, mut outs) = state;
        let (viewer, more) = unbounded(viewer, StageIn::Media(next), at, params);
        outs.extend(more);
        let (viewer, more) = overrun(viewer, window, at, params);
        outs.extend(more);
        state = (viewer, outs);
    }
    state
}

/// The inputs that stand for `input` inside `window`.
fn held_to(
    media: &MediaStage,
    window: TrimWindow,
    input: MediaIn,
    step: MediaTime,
) -> Vec<MediaIn> {
    let position = media.position();
    match input {
        MediaIn::Toggle => match media {
            MediaStage::Paused { at, .. } | MediaStage::Ended { at, .. }
                if *at >= window.end || *at < window.start =>
            {
                vec![MediaIn::SeekTo(window.start), MediaIn::Toggle]
            }
            MediaStage::Opening
            | MediaStage::Playing { .. }
            | MediaStage::Paused { .. }
            | MediaStage::Scrubbing { .. }
            | MediaStage::Ended { .. }
            | MediaStage::Failed(_) => vec![input],
        },
        MediaIn::SeekBack => match position {
            Some(from) => vec![MediaIn::SeekTo(
                window.clamp(MediaTime(from.0.saturating_sub(step.0))),
            )],
            None => vec![input],
        },
        MediaIn::SeekForward => match position {
            Some(from) => vec![MediaIn::SeekTo(
                window.clamp(MediaTime(from.0.saturating_add(step.0))),
            )],
            None => vec![input],
        },
        MediaIn::SeekTo(to) => vec![MediaIn::SeekTo(window.clamp(to))],
        MediaIn::ScrubTo(to) => vec![MediaIn::ScrubTo(window.clamp(to))],
        MediaIn::Player(_)
        | MediaIn::Position(_)
        | MediaIn::ScrubStart
        | MediaIn::ScrubEnd
        | MediaIn::ScrubCancel
        | MediaIn::SetVolume(_)
        | MediaIn::Select { kind: _, choice: _ }
        | MediaIn::FrameStep(_)
        | MediaIn::SetSpeed(_)
        | MediaIn::StepSpeed(_)
        | MediaIn::CycleTrack(_)
        | MediaIn::StepChapter(_)
        | MediaIn::GoToChapter(_)
        | MediaIn::Mark(_)
        | MediaIn::Restore { .. }
        | MediaIn::Failed(_)
        | MediaIn::Elapsed => vec![input],
    }
}

/// A player that is playing at or past the end of `window` is held at the end.
fn overrun(viewer: Viewer, window: TrimWindow, at: Stamp, params: &ViewerParams) -> Step {
    let past = matches!(
        &viewer.stage,
        Stage::Media(MediaStage::Playing { at: now, .. }) if *now >= window.end
    );
    if !past {
        return (viewer, Vec::new());
    }
    let (viewer, mut outs) = unbounded(viewer, StageIn::Media(MediaIn::Toggle), at, params);
    let (viewer, more) = unbounded(
        viewer,
        StageIn::Media(MediaIn::SeekTo(window.end)),
        at,
        params,
    );
    outs.extend(more);
    (viewer, outs)
}

/// What Return and Esc mean while the bar is up and nothing else has the keys.
pub(super) fn trim_key(viewer: &Viewer, keys: &[ShortcutKey]) -> Option<TrimIn> {
    let free = matches!(viewer.sheet, Sheet::Closed)
        && matches!(viewer.palette, Palette::Closed)
        && matches!(viewer.context, ContextMenu::Closed);
    if !free || !viewer.trim.is_choosing() {
        return None;
    }
    match keys {
        [ShortcutKey::Enter] => Some(TrimIn::Apply),
        [ShortcutKey::Escape] => Some(TrimIn::Cancel),
        _ => None,
    }
}

/// ⌘S on a recording with a trim applied: the cut is written over the file, the original kept as
/// a version. Without a trim there is nothing to save.
pub(super) fn save(viewer: Viewer, at: Stamp, params: &ViewerParams) -> Step {
    let Some(edit) = viewer.trim.edit() else {
        return (viewer, Vec::new());
    };
    let request = EditRequest::of_recording(edit);
    match params.sheet.edit {
        EditOffer::Plain => (viewer, vec![ViewerOut::Edit(request)]),
        EditOffer::Asks(caution) => sheet(viewer, SheetIn::AskEdit(request, caution), at, params),
        EditOffer::Copies(_) | EditOffer::Withheld => (viewer, Vec::new()),
    }
}
