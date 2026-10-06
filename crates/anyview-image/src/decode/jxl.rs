//! JPEG XL through `jxl-oxide`: the first frame, converted to sRGB.

use super::ceiling::Ceiling;
use super::colour::ColourInfo;
use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::{PixelLen, PixelSize};
use jxl_oxide::{EnumColourEncoding, JxlImage, RenderingIntent};
use std::io::Cursor;

fn decode_error(error: impl std::fmt::Display) -> ImageError {
    ImageError::Decode {
        reason: error.to_string(),
    }
}

/// What a render costs per pixel at its peak: the decoder's f32 RGBA channels, the f32 buffer
/// they are gathered into and the RGBA8 written from it.
const COST: u64 = 16 + 16 + 4;

/// The first frame of a JPEG XL file as straight RGBA8 in sRGB, with the file's orientation
/// already applied by the decoder, and the colour the file stored.
pub(crate) fn decode(bytes: &[u8], ceiling: Ceiling) -> Result<(Rgba8, ColourInfo), ImageError> {
    let mut image = JxlImage::builder()
        .read(Cursor::new(bytes))
        .map_err(decode_error)?;
    let declared = PixelSize {
        width: PixelLen(image.width()),
        height: PixelLen(image.height()),
    };
    ceiling.admit(declared, COST)?;
    let bits = image.image_header().metadata.bit_depth.bits_per_sample();
    image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
    let render = image.render_frame(0).map_err(decode_error)?;
    let buffer = render.image_all_channels();
    let channels = buffer.channels();
    let size = PixelSize {
        width: PixelLen(u32::try_from(buffer.width()).unwrap_or(u32::MAX)),
        height: PixelLen(u32::try_from(buffer.height()).unwrap_or(u32::MAX)),
    };
    let to_byte = |sample: f32| (sample.clamp(0.0, 1.0) * 255.0 + 0.5) as u8; // clamped to 0..=255
    let bytes: Vec<u8> = buffer
        .buf()
        .chunks_exact(channels.max(1))
        .flat_map(|pixel| match pixel {
            [grey] => [*grey, *grey, *grey, 1.0],
            [grey, alpha] => [*grey, *grey, *grey, *alpha],
            [red, green, blue] => [*red, *green, *blue, 1.0],
            [red, green, blue, alpha, ..] => [*red, *green, *blue, *alpha],
            [] => [0.0; 4],
        })
        .map(to_byte)
        .collect();
    let colour = ColourInfo::of_channels(
        u8::try_from(channels).unwrap_or(4),
        u8::try_from(bits).unwrap_or(u8::MAX),
    );
    Ok((Rgba8::new(size, bytes)?, colour))
}
