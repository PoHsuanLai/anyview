//! The peek of video and audio: libav's facts and, for an audio file that has one, its cover. A
//! recording is never decoded here, only its header read and its cover picture reduced to the pane's
//! size; the player (libmpv) is not in this crate's tree.

use crate::body::Body;
use crate::described::Described;
use crate::error::PeekError;
use anyview_core::{
    FactLabel, FactValue, Facts, FileHead, FileName, FormatKind, Peek, PeekBudget, PixelLen,
    Resize, SniffStep, Sniffed, Source, sniff,
};
use anyview_image::{
    Decoded, ExifFacts, FrameCount, ImagePeek, PeekedFormat, decode_bytes, resized,
};
use anyview_media::{CoverArt, CoverCodec, MediaPeeked};
use std::sync::Arc;

/// What a peek of a recording holds: what libav read, how the file is described, and the cover
/// when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLook {
    /// The file's type in words, for the first row of facts and for a recording with no cover.
    pub described: Described,
    /// What libav read.
    pub media: MediaPeeked,
    /// The cover picture, reduced to the budget.
    pub cover: Option<Arc<ImagePeek>>,
}

fn look(
    src: &Source,
    sniffed: &Sniffed,
    budget: &PeekBudget,
    read: impl FnOnce(&Source, &Sniffed, &PeekBudget) -> Result<MediaPeeked, anyview_media::MediaError>,
) -> Result<MediaLook, PeekError> {
    let media = read(src, sniffed, budget)?;
    let cover = media
        .probe
        .cover
        .as_ref()
        .and_then(|cover| cover_peek(cover, budget));
    Ok(MediaLook {
        described: Described::of(sniffed),
        media,
        cover: cover.map(Arc::new),
    })
}

/// The cover decoded and reduced to the pixels the budget allows; `None` when it will not decode,
/// since a cover is a nicety and the facts still stand.
fn cover_peek(cover: &CoverArt, budget: &PeekBudget) -> Option<ImagePeek> {
    let name = match cover.codec {
        CoverCodec::Png => "cover.png",
        CoverCodec::Jpeg => "cover.jpg",
    };
    let head = FileHead::new(&cover.bytes[..cover.bytes.len().min(4096)]);
    let SniffStep::Done(sniffed) = sniff(&head, &FileName::new(name).ok()?) else {
        return None;
    };
    let Decoded::Still(picture) = decode_bytes(&cover.bytes, &sniffed).ok()? else {
        return None;
    };
    let source_size = picture.size();
    let area = budget.pixels.0.min(budget.bytes.0 / 4);
    // The longest edge a square of the budget's pixels allows.
    let edge = (area as f64).sqrt() as u32;
    let long = source_size.width.0.max(source_size.height.0);
    let picture = if edge > 0 && long > edge {
        resized(&picture, Resize::LongEdge(PixelLen(edge)))
    } else {
        picture
    };
    let anyview_core::FormatDetail::Raster(format) = sniffed.detail() else {
        return None;
    };
    Some(ImagePeek {
        picture,
        source_size,
        frames: FrameCount(1),
        colour: None,
        exif: ExifFacts::none(),
        format: PeekedFormat::Raster(*format),
    })
}

/// The kind row, then the rows libav gave.
fn rows(look: &MediaLook, media: fn(&MediaPeeked) -> Facts) -> Facts {
    media(&look.media).rows().iter().fold(
        Facts::empty().with(
            FactLabel::Kind,
            FactValue::text(look.described.kind.clone()),
        ),
        |facts, row| facts.with(row.label, row.value.clone()),
    )
}

/// The peek of a video file: facts only, no poster.
#[derive(Debug, Clone, Copy)]
pub struct VideoPeek;

impl Peek for VideoPeek {
    const KIND: FormatKind = FormatKind::Video;
    type Peeked = MediaLook;
    type Error = PeekError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<MediaLook, PeekError> {
        look(src, sniffed, budget, |src, sniffed, budget| {
            anyview_media::VideoPeek::peek(src, sniffed, budget)
        })
    }

    fn facts(peeked: &MediaLook) -> Facts {
        rows(peeked, anyview_media::VideoPeek::facts)
    }
}

/// The peek of an audio file: facts and the cover it carries.
#[derive(Debug, Clone, Copy)]
pub struct AudioPeek;

impl Peek for AudioPeek {
    const KIND: FormatKind = FormatKind::Audio;
    type Peeked = MediaLook;
    type Error = PeekError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<MediaLook, PeekError> {
        look(src, sniffed, budget, |src, sniffed, budget| {
            anyview_media::AudioPeek::peek(src, sniffed, budget)
        })
    }

    fn facts(peeked: &MediaLook) -> Facts {
        rows(peeked, anyview_media::AudioPeek::facts)
    }
}

impl From<MediaLook> for Body {
    fn from(peeked: MediaLook) -> Self {
        match peeked.cover {
            Some(cover) => Body::Picture(cover),
            None => Body::FactsOnly(peeked.described),
        }
    }
}
