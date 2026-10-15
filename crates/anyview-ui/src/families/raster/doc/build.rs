//! Building a picture's document: its name and facts, and uploading its pixels to the texture.

use super::types::{FrameStrip, RasterDoc, RasterOpen, StripFrame};
use crate::io::OpenError;
use crate::{EditOffer, Runs};
use anyview_core::{
    FactLabel, FactValue, Facts, FormatKind, PixelLen, PixelSize, Resize, Sniffed, Source,
};
use anyview_image::{Animation, Rgba8, picture_facts, resized};
use ds_blitz::{PixelFormat, Pixels, TextureHandle};
use std::sync::Arc;
use std::time::Duration;

/// The name of the file `src`, as the card of a picture that cannot be shown calls it.
pub(super) fn name_of(src: &Source) -> String {
    src.path()
        .file_name()
        .map_or_else(String::new, |name| name.as_str().to_owned())
}
pub(super) fn doc_of(
    target: &RasterOpen,
    bytes: &[u8],
    size: PixelSize,
    frames: u32,
    strip: Option<Arc<FrameStrip>>,
    runs: Runs,
) -> RasterDoc {
    RasterDoc {
        name: name_of(&target.source),
        texture: target.texture.clone(),
        size,
        held: size,
        frames,
        facts: facts(size, frames).then(metadata_facts(bytes, &target.sniffed)),
        needs: None,
        lacking: None,
        offer: EditOffer::Plain,
        strip,
        runs,
    }
}
/// Every frame of `animation` in a texture of its own: the first goes into `first`, the others
/// into new handles on the same GPU.
pub(super) fn upload_frames(
    first: &TextureHandle,
    animation: &Animation,
) -> Result<(FrameStrip, PixelSize), OpenError> {
    let mut frames = Vec::new();
    let mut held = animation.frames.first().pixels.size();
    for (index, frame) in animation.frames.iter().enumerate() {
        let texture = if index == 0 {
            first.clone()
        } else {
            first.gpu().handle()
        };
        held = upload(&texture, &frame.pixels)?;
        frames.push(StripFrame {
            texture,
            delay: Duration::from_micros(frame.delay.0),
        });
    }
    Ok((FrameStrip { frames }, held))
}
/// Puts `picture` in `texture` and says what size it holds. A picture longer on a side than the
/// GPU's texture limit goes up shrunk to fit it, as a viewer shows a huge image; the doc keeps
/// the picture's own size, so the zoom and the facts stay true.
pub(super) fn upload(texture: &TextureHandle, picture: &Rgba8) -> Result<PixelSize, OpenError> {
    let limit = texture
        .gpu()
        .device()
        .map_or(u32::MAX, |device| device.limits().max_texture_dimension_2d)
        // A device that reports no room at all would shrink the picture for ever.
        .max(1);
    let size = picture.size();
    if size.width.0.max(size.height.0) > limit {
        return upload(
            texture,
            &resized(picture, Resize::LongEdge(PixelLen(limit))),
        );
    }
    let pixels = Pixels::new(
        PixelFormat::Rgba8Straight,
        size.width.0,
        size.height.0,
        picture.bytes(),
    )
    .map_err(|_| OpenError::Unrecognised)?;
    texture.update(&pixels).map_err(OpenError::Gpu)?;
    Ok(size)
}
/// The rows of the Info tab that need no file: the picture's size and, for an animation, its
/// frames. The General section (kind, size, dates) is the window's.
pub(super) fn facts(size: PixelSize, frames: u32) -> Facts {
    let facts = Facts::empty().with(FactLabel::Dimensions, FactValue::dimensions(size));
    if frames > 1 {
        facts.with(FactLabel::Frames, FactValue::text(frames.to_string()))
    } else {
        facts
    }
}
/// Colour, density, camera and location, read from the file's own headers in `bytes`. An SVG has
/// none.
fn metadata_facts(bytes: &[u8], sniffed: &Sniffed) -> Facts {
    if sniffed.kind() == FormatKind::Raster {
        picture_facts(bytes, sniffed)
    } else {
        Facts::empty()
    }
}
/// How much of a file the cheap first look reads for what it says about itself: the EXIF block of
/// a JPEG, PNG, WebP or HEIC sits in the first blocks, and a TIFF's in most files.
const FIRST_LOOK_METADATA_READ: u64 = 32 * 1024 * 1024;
/// [`metadata_facts`] of the start of the file at `source`, for the first look that has not read
/// the file whole. A file that cannot be read has none.
pub(super) fn file_metadata_facts(source: &Source, sniffed: &Sniffed) -> Facts {
    use std::io::Read;
    let mut bytes = Vec::new();
    let read = std::fs::File::open(source.path().as_path())
        .and_then(|file| file.take(FIRST_LOOK_METADATA_READ).read_to_end(&mut bytes));
    match read {
        Ok(_) => metadata_facts(&bytes, sniffed),
        Err(_) => Facts::empty(),
    }
}
