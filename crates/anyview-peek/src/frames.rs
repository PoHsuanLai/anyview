//! The seam for a video's frame. Showing a picture of a video would take a decoder, and the launcher
//! links none; but the desktop's thumbnail cache usually holds one that the file manager or a
//! thumbnailer made, and reading it is the host's business (the cache sits behind D-Bus-free but
//! platform-specific paths that `anyview-platform` owns, which this crate may not name). So the host
//! hands [`peek_with`](crate::peek_with) a [`VideoFrames`], and a video whose header gave no cover
//! shows its frame; without one, or without a cached thumbnail, the pane shows the facts card.

use anyview_core::{
    FileHead, FileName, FormatDetail, Input, PeekBudget, PixelLen, RasterFormat, Resize, SniffStep,
    sniff,
};
use anyview_image::{
    Decoded, ExifFacts, FrameCount, ImagePeek, PeekedFormat, Rgba8, decode_bytes, resized,
};
use std::fmt::Debug;

/// Where the host finds a picture of a video it did not decode.
pub trait VideoFrames: Debug + Send + Sync {
    /// The small picture of `source` as it is now (a cached thumbnail made for this very version of
    /// the file; `source.path()` says where it is, when it is a file at all), upright, or `None` when the host has none. Blocking: it reads a file.
    fn frame(&self, source: &Input) -> Option<Rgba8>;
}

/// No host source: every video without a cover shows its facts.
#[derive(Debug, Clone, Copy)]
pub struct NoFrames;

impl VideoFrames for NoFrames {
    fn frame(&self, _: &Input) -> Option<Rgba8> {
        None
    }
}

/// `picture` reduced so it fits the pixels `budget` allows.
pub(crate) fn reduced(picture: Rgba8, budget: &PeekBudget) -> Rgba8 {
    let area = budget.pixels.0.min(budget.bytes.0 / 4);
    // The longest edge a square of the budget's pixels allows.
    let edge = (area as f64).sqrt() as u32;
    let size = picture.size();
    let long = size.width.0.max(size.height.0);
    if edge > 0 && long > edge {
        resized(&picture, Resize::LongEdge(PixelLen(edge)))
    } else {
        picture
    }
}

/// The peek of a still `picture` (a thumbnail is a PNG) reduced to `budget`.
pub(crate) fn still_peek(picture: Rgba8, budget: &PeekBudget) -> ImagePeek {
    let source_size = picture.size();
    ImagePeek {
        picture: reduced(picture, budget),
        source_size,
        frames: FrameCount(1),
        colour: None,
        exif: ExifFacts::none(),
        format: PeekedFormat::Raster(RasterFormat::Png),
    }
}

/// The still picture in the image file `bytes` (`name` says what format it is, since a cover has
/// no file of its own), reduced to `budget`. `None` when it will not decode or is an animation:
/// a cover is a nicety and the facts still stand.
pub(crate) fn cover_picture(bytes: &[u8], name: &str, budget: &PeekBudget) -> Option<ImagePeek> {
    let head = FileHead::new(&bytes[..bytes.len().min(4096)]);
    let SniffStep::Done(sniffed) = sniff(&head, &FileName::new(name).ok()?) else {
        return None;
    };
    let Decoded::Still(picture) = decode_bytes(bytes, &sniffed).ok()? else {
        return None;
    };
    let source_size = picture.size();
    let picture = reduced(picture, budget);
    let FormatDetail::Raster(format) = sniffed.detail() else {
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
