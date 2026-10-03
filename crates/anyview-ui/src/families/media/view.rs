//! The recording on screen: a room the size of the stage holding the picture the player draws, or
//! the card of an audio file. The view tells the player how large the room is and sends the
//! stage machine a toggle when it is clicked; it decides nothing else.

use super::audio::accent_for;
use super::doc::MediaDoc;
use crate::families::view::{Area, Held, StageCx};
use crate::io::SlotPixels;
use crate::{MediaError, MediaIn, MediaStage, Stage, StageIn};
use anyview_core::VideoPresence;
use dioxus::prelude::*;
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
        | Stage::Text(_) => None,
    }
}

#[component]
pub(super) fn MediaContent(doc: Held<MediaDoc>, cx: StageCx) -> Element {
    let line = std::sync::Arc::clone(doc.0.line());
    let slot = cx.area.and_then(slot_of);
    // The player draws into a rectangle the size of the room; it is told whenever that changes.
    use_effect(use_reactive!(|slot| line.resize(slot)));
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
