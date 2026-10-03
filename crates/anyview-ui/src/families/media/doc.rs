//! A recording opened: the player that was started for it and what the host read of the file.

use crate::Ticket;
use crate::io::{MediaLine, OpenError, OpenLink};
use anyview_core::{FactLabel, FactValue, Facts, FormatKind, MediaTags, Sniffed, Source};
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
    pub(super) line: Arc<dyn MediaLine>,
    pub(super) texture: TextureHandle,
    pub(super) facts: Facts,
}

impl MediaDoc {
    /// The line to the player.
    pub fn line(&self) -> &Arc<dyn MediaLine> {
        &self.line
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
    let started = port.start(ticket, src.path().clone(), link.texture.clone())?;
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
        .with(FactLabel::Size, FactValue::size(src.stamp().len));
    let facts = started
        .facts
        .rows()
        .iter()
        .fold(facts, |facts, row| facts.with(row.label, row.value.clone()));
    Ok(MediaDoc {
        name: src
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned()),
        kind: sniffed.kind(),
        tags: started.tags,
        line: started.line,
        texture: link.texture.clone(),
        facts,
    })
}
