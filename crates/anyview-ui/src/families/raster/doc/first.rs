//! The first frame of a picture, drawn from a cheap read before the full open is in.

use super::build::{facts, file_metadata_facts, name_of, upload};
use super::types::RasterDoc;
use crate::io::{OpenError, OpenPort};
use crate::{EditOffer, Runs};
use anyview_core::{
    ByteLen, Facts, FormatDetail, FormatKind, PeekBudget, PixelArea, PixelSize, RasterFormat,
    Sniffed, Source,
};
use anyview_image::{ImagePeek, RasterPeek, Rgba8, VectorPeek, declared_size};
use std::time::Duration;

/// What a first frame may spend: a picture about the size of a window, from a file read whole.
/// The time is the pool's to enforce; the decoder reads no clock.
const FIRST_FRAME: PeekBudget = PeekBudget::DEFAULT
    .with_bytes(ByteLen(64 * 1024 * 1024))
    .with_pixels(PixelArea(4 * 1024 * 1024))
    .with_time(Duration::from_secs(1));
/// The first frame of a picture, cheaper than decoding it whole: the host's small picture of the
/// file when it has one (given the size the file declares, so the box it fills is the final one),
/// otherwise the first frame of an animation or a drawn-small vector image. `None` for a picture
/// that would take as long to peek as to open.
pub(crate) fn first_frame(
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenPort,
) -> Result<Option<RasterDoc>, OpenError> {
    let Some((picture, size, frames, facts)) = cheap_picture(src, sniffed, link)? else {
        return Ok(None);
    };
    let held = upload(&link.texture, &picture)?;
    Ok(Some(RasterDoc {
        name: name_of(src),
        texture: link.texture.clone(),
        size,
        held,
        frames,
        facts,
        needs: None,
        lacking: None,
        offer: EditOffer::Plain,
        strip: None,
        runs: Runs::Forever,
    }))
}
type Cheap = (Rgba8, PixelSize, u32, Facts);
fn cheap_picture(
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenPort,
) -> Result<Option<Cheap>, OpenError> {
    if let Some(picture) = link.first_frames.still(&src.on_disk()) {
        let size = declared_size(src.on_disk(), sniffed)?.unwrap_or_else(|| picture.size());
        let facts = facts(size, 1).then(file_metadata_facts(src, sniffed));
        return Ok(Some((picture, size, 1, facts)));
    }
    let (peeked, facts): (ImagePeek, Facts) = match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Vector, _) => {
            let peeked = VectorPeek::peek(&src.on_disk(), sniffed, &FIRST_FRAME)?;
            let facts = VectorPeek::facts(&peeked);
            (peeked, facts)
        }
        // A raw file's embedded preview is a cheap first frame while a plugin develops the whole.
        (
            FormatKind::Raster,
            FormatDetail::Raster(RasterFormat::Gif | RasterFormat::Webp | RasterFormat::Raw),
        ) => {
            let peeked = RasterPeek::peek(&src.on_disk(), sniffed, &FIRST_FRAME)?;
            let facts = RasterPeek::facts(&peeked);
            (peeked, facts)
        }
        _ => return Ok(None),
    };
    let frames = peeked.frames.0;
    Ok(Some((peeked.picture, peeked.source_size, frames, facts)))
}
