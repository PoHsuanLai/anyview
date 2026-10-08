//! The recording on screen: a room the size of the stage holding the picture the player draws, or
//! the cover of an audio file. The view tells the player how large the room is and sends the
//! stage machine a toggle when it is clicked; it decides nothing else.

use super::doc::MediaDoc;
use crate::families::view::{Area, Held, StageCx};
use crate::io::SlotPixels;
use crate::{Command, MediaError, MediaIn, MediaStage, Need, Stage, StageIn};
use anyview_core::{FileAction, FormatKind, VideoPresence};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::button::Button;
use ds::components::fields::fact_list::FactList;
use ds::components::overlays::empty_state::EmptyState;
use ds::prelude::{Icon, IconSource, IconView};
use ds::style::icon::render::{IconPx, IconSize};
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

/// The side of the music symbol on the tile of an audio file with no cover, in logical pixels.
const SYMBOL_SIDE: u8 = 64;

/// `Artist · Album`, or whichever the file has.
fn byline(tags: &anyview_core::MediaTags) -> Option<String> {
    match (&tags.artist, &tags.album) {
        (Some(artist), Some(album)) => Some(format!("{artist} \u{b7} {album}")),
        (Some(one), None) | (None, Some(one)) => Some(one.clone()),
        (None, None) => None,
    }
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
    let cover = held.cover().cloned();
    let audio = held.kind == FormatKind::Audio;
    let shows_picture = match picture {
        VideoPresence::CoverArt | VideoPresence::Present => true,
        VideoPresence::Absent => false,
    };
    let word = status(&cx.stage);
    rsx! {
        div {
            class: "viewer-media",
            "data-picture": picture.slug(),
            if shows_picture {
                TextureLayer {
                    texture: Some(held.texture.clone()),
                    fit: TextureFit::Fill,
                    sampling: Sampling::Nearest,
                }
            } else {
                div { class: "viewer-media-sleeve",
                    if audio {
                        if let Some(image) = cover {
                            img { class: "viewer-media-art", alt: "Cover art", src: image.0 }
                        } else {
                            div { class: "viewer-media-art viewer-media-tile",
                                IconView {
                                    source: IconSource::from(Icon::Music),
                                    size: IconSize::Px(IconPx(SYMBOL_SIDE)),
                                }
                            }
                        }
                    }
                    h1 { class: "viewer-media-title", "{held.title()}" }
                    if let Some(line) = byline(&held.tags) {
                        p { class: "viewer-media-byline", "{line}" }
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

/// A recording no plugin plays: its facts, the package that would play it, and Show in Folder.
#[component]
fn Unplayable(doc: Held<MediaDoc>, needs: Need, cx: StageCx) -> Element {
    let run = cx.run;
    let platform = cx.platform;
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
                            if platform.offers(FileAction::RevealInFolder) {
                                Button {
                                    label: "Show in Folder",
                                    onclick: move |_| run.call(Command::File(FileAction::RevealInFolder)),
                                }
                            }
                        }
                    },
                }
                div { class: "viewer-peek-facts", FactList { facts } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::MediaTags;

    #[test]
    fn the_byline_joins_artist_and_album_with_a_dot() {
        let tags = |artist: Option<&str>, album: Option<&str>| MediaTags {
            title: None,
            artist: artist.map(str::to_owned),
            album: album.map(str::to_owned),
        };
        assert_eq!(
            byline(&tags(Some("A"), Some("B"))).as_deref(),
            Some("A \u{b7} B")
        );
        assert_eq!(byline(&tags(Some("A"), None)).as_deref(), Some("A"));
        assert_eq!(byline(&tags(None, Some("B"))).as_deref(), Some("B"));
        assert_eq!(byline(&tags(None, None)), None);
    }
}
