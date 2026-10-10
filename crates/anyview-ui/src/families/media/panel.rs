//! The side panel's tabs for a recording: the tracks and the speed, and the chapters. Every row
//! hands the stage machine an input; the lists are what the player last reported.

use super::doc::MediaDoc;
use super::live::MediaLive;
use crate::families::view::{Held, StageCx};
use crate::{MediaIn, PanelTab, StageIn, TrackKind};
use anyview_core::{
    ChapterIndex, MediaTime, MediaTrack, Speed, StreamKind, TrackChoice, TrackPlay,
};
use dioxus::prelude::*;
use ds::components::controls::segmented::Tracking;
use ds::prelude::{Choice, Row, SegmentedControl};
use ds_core::vocab::{RowState, Selection};
use std::sync::Arc;

/// The panel's body for `tab`, when the recording has something for it beyond its facts.
pub(super) fn body(doc: &Arc<MediaDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
    match tab {
        PanelTab::Tracks => Some(rsx! { Tracks { doc: Held(Arc::clone(doc)), cx: cx.clone() } }),
        PanelTab::Contents => {
            Some(rsx! { Chapters { doc: Held(Arc::clone(doc)), cx: cx.clone() } })
        }
        PanelTab::Info | PanelTab::Thumbnails | PanelTab::Sheets => None,
    }
}

/// `1.25×`: a speed as a person says it.
pub(super) fn speed_text(speed: Speed) -> String {
    let thousandths = speed.thousandths();
    let (whole, fraction) = (thousandths / 1000, thousandths % 1000);
    let digits = format!("{fraction:03}");
    let digits = digits.trim_end_matches('0');
    match digits.is_empty() {
        true => format!("{whole}×"),
        false => format!("{whole}.{digits}×"),
    }
}

/// `1:05`, or `1:02:03` from one hour.
pub(super) fn clock_text(at: MediaTime) -> String {
    let secs = at.0 / 1_000_000;
    let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn selected(is: bool) -> Selection {
    match is {
        true => Selection::Selected,
        false => Selection::Unselected,
    }
}

fn kind_of(kind: StreamKind) -> Option<TrackKind> {
    match kind {
        StreamKind::Audio => Some(TrackKind::Audio),
        StreamKind::Subtitles => Some(TrackKind::Subtitles),
        StreamKind::Video => None,
    }
}

fn pick(kind: TrackKind, choice: TrackChoice) -> StageIn {
    StageIn::Media(MediaIn::Select { kind, choice })
}

#[component]
fn Tracks(doc: Held<MediaDoc>, cx: StageCx) -> Element {
    let live = cx.media.read();
    let send = cx.send;
    let speeds: Vec<Choice<Speed>> = Speed::PRESETS
        .iter()
        .map(|speed| Choice::new(*speed, speed_text(*speed)))
        .collect();
    let current = live.speed;
    rsx! {
        div { class: "viewer-tracks",
            p { class: "viewer-tracks-heading", "Speed" }
            SegmentedControl::<Speed> {
                label: "Speed",
                choices: speeds,
                tracking: Tracking::SelectOne(current),
                onchange: move |speed: Speed| send.call(StageIn::Media(MediaIn::SetSpeed(speed))),
            }
            {track_list(&live, StreamKind::Audio, "Audio", send)}
            {track_list(&live, StreamKind::Subtitles, "Subtitles", send)}
        }
    }
}

/// One kind's heading and rows; subtitles have a row for none.
fn track_list(
    live: &MediaLive,
    kind: StreamKind,
    heading: &str,
    send: EventHandler<StageIn>,
) -> Element {
    let Some(which) = kind_of(kind) else {
        return rsx! {};
    };
    let tracks: Vec<&MediaTrack> = live.of_kind(kind).collect();
    if tracks.is_empty() {
        return rsx! {};
    }
    let none_playing = tracks.iter().all(|track| track.play == TrackPlay::Idle);
    let offers_off = matches!(kind, StreamKind::Subtitles);
    rsx! {
        p { class: "viewer-tracks-heading", "{heading}" }
        if offers_off {
            Row {
                title: "Off",
                state: RowState { selection: selected(none_playing), ..RowState::default() },
                onclick: move |_| send.call(pick(which, TrackChoice::Off)),
            }
        }
        for track in tracks {
            {
                let (id, playing) = (track.id, track.play == TrackPlay::Playing);
                rsx! {
                    Row {
                        key: "{which:?}-{id.0}",
                        title: track.name(),
                        state: RowState { selection: selected(playing), ..RowState::default() },
                        onclick: move |_| send.call(pick(which, TrackChoice::Track(id))),
                    }
                }
            }
        }
    }
}

#[component]
fn Chapters(doc: Held<MediaDoc>, cx: StageCx) -> Element {
    let live = cx.media.read();
    let send = cx.send;
    if live.chapters.is_empty() {
        return rsx! { p { class: "viewer-tracks-heading", "This recording has no chapters." } };
    }
    let now = live.position.and_then(|position| live.chapter_at(position));
    rsx! {
        div { class: "viewer-chapters", role: "list",
            for (at, chapter) in live.chapters.iter().enumerate() {
                {
                    let index = ChapterIndex(u32::try_from(at).unwrap_or(u32::MAX));
                    let title = if chapter.title.is_empty() {
                        format!("Chapter {}", at + 1)
                    } else {
                        chapter.title.clone()
                    };
                    let state = RowState { selection: selected(now == Some(at)), ..RowState::default() };
                    rsx! {
                        Row {
                            key: "{at}",
                            title,
                            state,
                            onclick: move |_| send.call(StageIn::Media(MediaIn::GoToChapter(index))),
                            detail: Some(clock_text(chapter.start).into()),
                        }
                    }
                }
            }
        }
    }
}
