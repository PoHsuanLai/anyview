//! A recording opened: the player that was started for it and what the host read of the file.

use crate::MediaOffer;
use crate::Ticket;
use crate::io::{MediaLine, MediaPlayback, OpenError, OpenLink};
use anyview_core::{
    FactGroup, FactLabel, FactValue, Facts, FormatKind, MediaTags, Sniffed, Source,
};
use ds::components::content::image_source::ImageSource;
use ds_blitz::TextureHandle;
use std::sync::Arc;

/// A video or audio file, playing. Dropping the last holder of the document ends the player, so
/// a window that closes, or a file the person walks away from, silences itself.
#[derive(Debug, Clone)]
pub struct MediaDoc {
    /// What the file is called.
    pub name: String,
    /// Video or audio.
    pub kind: FormatKind,
    /// What the file says of itself.
    pub tags: MediaTags,
    pub(super) playback: MediaPlayback,
    pub(super) offer: MediaOffer,
    pub(super) texture: TextureHandle,
    pub(super) facts: Facts,
    pub(super) cover: Option<ImageSource>,
}

impl MediaDoc {
    /// The line to the player, when one plays the recording.
    pub fn line(&self) -> Option<&Arc<dyn MediaLine>> {
        match &self.playback {
            MediaPlayback::Line(line) => Some(line),
            MediaPlayback::Missing(_) => None,
        }
    }

    /// The package that would play the recording, when none does: a facts card is what it shows.
    pub fn needs(&self) -> Option<&crate::Need> {
        match &self.playback {
            MediaPlayback::Line(_) => None,
            MediaPlayback::Missing(needs) => Some(needs),
        }
    }

    /// The picture an audio file carries, when it carries one.
    pub fn cover(&self) -> Option<&ImageSource> {
        self.cover.as_ref()
    }

    /// The exports on offer for the recording.
    pub fn offer(&self) -> &MediaOffer {
        &self.offer
    }

    /// The title to show large: the tag, else the name of the file.
    pub fn title(&self) -> &str {
        self.tags.title.as_deref().unwrap_or(&self.name)
    }
}

/// Start the player for `src` and describe the file.
pub(super) fn open(
    ticket: Ticket,
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<MediaDoc, OpenError> {
    let port = link
        .media
        .as_ref()
        .ok_or_else(|| OpenError::Media("a file opened ahead does not play".to_owned()))?;
    let started = port.start(ticket, src, sniffed, link.texture.clone())?;
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
        .with(FactLabel::Size, FactValue::size(src.stamp().len));
    let facts = started.facts.rows().iter().fold(facts, |facts, row| {
        // A recording's size is its picture's: the Video section, not an image's Picture.
        let row = row.clone();
        facts.with_fact(if row.label == FactLabel::Dimensions {
            row.in_group(FactGroup::Video)
        } else {
            row
        })
    });
    let facts = match &started.playback {
        MediaPlayback::Missing(needs) => facts.with(needs.fact.label, needs.fact.value.clone()),
        MediaPlayback::Line(_) => facts,
    };
    Ok(MediaDoc {
        name: src
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned()),
        kind: sniffed.kind(),
        tags: started.tags,
        playback: started.playback,
        offer: started.offer,
        texture: link.texture.clone(),
        facts,
        cover: started.cover,
    })
}
