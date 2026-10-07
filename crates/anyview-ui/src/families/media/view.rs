//! The recording on screen: a room the size of the stage holding the picture the player draws, or
//! the card of an audio file. The view tells the player how large the room is and sends the
//! stage machine a toggle when it is clicked; it decides nothing else.

use super::audio::accent_for;
use super::doc::MediaDoc;
use crate::families::view::{Area, Held, StageCx};
use crate::io::SlotPixels;
use crate::{Command, MediaError, MediaIn, MediaStage, Need, Stage, StageIn};
use anyview_core::FileAction;
use anyview_core::VideoPresence;
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::button::Button;
use ds::components::fields::fact_list::FactList;
use ds::components::overlays::empty_state::EmptyState;
use ds::prelude::Icon;
use ds_blitz::{Sampling, TextureFit, TextureLayer};
use ds_core::word::Word;
use std::num::NonZeroU32;

/// The room as the player's slot: physical pixels, at least one each way.
fn slot_of(area: Area) -> Option<SlotPixels> {
    let pixels = |logical: f32| NonZeroU32::new((logical * area.scale).round().max(0.0) as u32);
    Some(SlotPixels {
        width: pixels(area.size.width.0)?,
        height: pixels(area.size.height.0)?,
    })
}

/// A word for what the stage is doing, shown while there is nothing else to show.
fn status(stage: &Stage) -> Option<&'static str> {
    match stage {
        Stage::Media(MediaStage::Opening) => Some("Opening"),
        Stage::Media(MediaStage::Failed(MediaError::OpenFailed)) => {
            Some("This recording cannot be played")
        }
        Stage::Media(MediaStage::Failed(MediaError::PlaybackFailed)) => Some("The player stopped"),
        Stage::Media(
            MediaStage::Playing { .. }
            | MediaStage::Paused { .. }
            | MediaStage::Scrubbing { .. }
            | MediaStage::Ended { .. },
        )
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Book(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}

#[component]
pub(super) fn MediaContent(doc: Held<MediaDoc>, cx: StageCx) -> Element {
    let line = doc.0.line().map(std::sync::Arc::clone);
    let slot = cx.area.and_then(slot_of);
    // The player draws into a rectangle the size of the room; it is told whenever that changes.
    use_effect(use_reactive!(|slot| {
        if let Some(line) = &line {
            line.resize(slot);
        }
    }));
    if let Some(needs) = doc.0.needs() {
        return rsx! { Unplayable { doc: doc.clone(), needs: needs.clone(), cx: cx.clone() } };
    }
    let live = cx.media.read();
    let picture = live.picture;
    drop(live);
    let send = cx.send;
    let held = &doc.0;
    let accent = accent_for(&[
        held.title(),
        held.tags.artist.as_deref().unwrap_or(""),
        held.tags.album.as_deref().unwrap_or(""),
    ]);
    let shows_picture = match picture {
        VideoPresence::CoverArt | VideoPresence::Present => true,
        VideoPresence::Absent => false,
    };
    let word = status(&cx.stage);
    rsx! {
        div {
            class: "viewer-media",
            "data-picture": picture.slug(),
            "data-accent": accent.slug(),
            if shows_picture {
                TextureLayer {
                    texture: Some(held.texture.clone()),
                    fit: TextureFit::Fill,
                    sampling: Sampling::Nearest,
                }
            } else {
                div { class: "viewer-media-card",
                    h1 { class: "viewer-media-title", "{held.title()}" }
                    if let Some(artist) = &held.tags.artist {
                        p { class: "viewer-media-artist", "{artist}" }
                    }
                    if let Some(album) = &held.tags.album {
                        p { class: "viewer-media-album", "{album}" }
                    }
                }
            }
            if let Some(word) = word {
                p { class: "viewer-media-status", "{word}" }
            }
            // A texture layer swallows a click, so a click on the picture would never toggle
            // playback; this cover is what the pointer lands on.
            div {
                class: "viewer-media-cover",
                onclick: move |_| send.call(StageIn::Media(MediaIn::Toggle)),
            }
        }
    }
}

/// A recording no plugin plays: its facts, the package that would play it, and Open With….
#[component]
fn Unplayable(doc: Held<MediaDoc>, needs: Need, cx: StageCx) -> Element {
    let run = cx.run;
    let facts: Vec<ds::components::fields::fact_list::Fact> = doc
        .0
        .facts
        .rows()
        .iter()
        // The package that plays it is the line under the title; it is not listed twice.
        .filter(|row| row.label != needs.fact.label)
        .map(|row| {
            ds::components::fields::fact_list::Fact::new(row.label.label(), row.value.as_str())
        })
        .collect();
    rsx! {
        div { class: "viewer-peek",
            div { class: "viewer-peek-body",
                EmptyState {
                    icon: Icon::File,
                    title: doc.0.title().to_owned(),
                    description: Some(TextLine::from(format!("{}: {}", needs.fact.label.label(), needs.fact.value.as_str()))),
                    action: rsx! {
                        div { class: "viewer-failed-actions",
                            if let Some(helper) = needs.helper {
                                Button {
                                    label: "Install…",
                                    onclick: move |_| run.call(Command::Install(helper)),
                                }
                            }
                            Button {
                                label: "Open With…",
                                onclick: move |_| run.call(Command::File(FileAction::OpenWith)),
                            }
                            Button {
                                label: "Show in Folder",
                                onclick: move |_| run.call(Command::File(FileAction::RevealInFolder)),
                            }
                        }
                    },
                }
                div { class: "viewer-peek-facts", FactList { facts } }
            }
        }
    }
}
