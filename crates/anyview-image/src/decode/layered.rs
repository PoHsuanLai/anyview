//! The two container formats `image` does not read: Photoshop documents (their flattened
//! composite, not the layers) and Apple icon families (their largest picture).

use super::MAX_DECODE_AREA;
use super::colour::{ColourInfo, ColourModel};
use crate::error::ImageError;
use crate::pixels::Rgba8;
use anyview_core::{PixelLen, PixelSize};
use icns::{Encoding, IconFamily, IconType, PixelFormat};
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn decode_error(reason: impl std::fmt::Display) -> ImageError {
    ImageError::Decode {
        reason: reason.to_string(),
    }
}

/// The size a Photoshop file declares: its header is a fixed 26 bytes (signature, version,
/// channels, then height and width as big-endian words).
pub(crate) fn psd_size(bytes: &[u8]) -> Result<PixelSize, ImageError> {
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_be_bytes)
    };
    match (bytes.get(..4), word(14), word(18)) {
        (Some(b"8BPS"), Some(height), Some(width)) => Ok(PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        }),
        _ => Err(decode_error("not a Photoshop header")),
    }
}

/// The flattened picture a Photoshop file stores beside its layers. The `psd` crate indexes and
/// unwraps on bytes it has not checked, so a file that breaks it is reported as undecodable
/// instead of taking the worker down.
pub(crate) fn psd(bytes: &[u8]) -> Result<(Rgba8, ColourInfo), ImageError> {
    let size = psd_size(bytes)?;
    if size.area() > MAX_DECODE_AREA {
        return Err(ImageError::TooLarge { size });
    }
    let flattened = catch_unwind(AssertUnwindSafe(|| {
        let document = ::psd::Psd::from_bytes(bytes).map_err(|e| decode_error(format!("{e:?}")))?;
        Ok((document.width(), document.height(), document.rgba()))
    }))
    .map_err(|_| decode_error("the Photoshop file is damaged"))??;
    let (width, height, pixels) = flattened;
    let size = PixelSize {
        width: PixelLen(width),
        height: PixelLen(height),
    };
    let colour = ColourInfo {
        model: ColourModel::Rgba,
        bits: 8,
    };
    Ok((Rgba8::new(size, pixels)?, colour))
}

/// One entry of an icon family as a picture. The `icns` crate is built without its own PNG reader,
/// so a PNG-packed entry goes through `image`; a JPEG 2000 one is skipped, as no decoder here reads
/// it.
fn icon_picture(family: &IconFamily, kind: IconType) -> Option<Rgba8> {
    let (width, height) = (kind.pixel_width(), kind.pixel_height());
    if kind.encoding() == Encoding::JP2PNG {
        let element = family.elements.iter().find(|e| e.ostype == kind.ostype())?;
        let decoded =
            image::load_from_memory_with_format(&element.data, image::ImageFormat::Png).ok()?;
        return (decoded.width() == width && decoded.height() == height)
            .then(|| Rgba8::from_image(decoded.into_rgba8()));
    }
    let image = family
        .get_icon_with_type(kind)
        .ok()?
        .convert_to(PixelFormat::RGBA);
    let size = PixelSize {
        width: PixelLen(image.width()),
        height: PixelLen(image.height()),
    };
    Rgba8::new(size, image.into_data().into_vec()).ok()
}

/// The largest picture of an icon family that can be decoded.
pub(crate) fn icns(bytes: &[u8]) -> Result<(Rgba8, ColourInfo), ImageError> {
    let family = IconFamily::read(Cursor::new(bytes)).map_err(decode_error)?;
    let mut kinds = family.available_icons();
    kinds.sort_by_key(|kind| {
        std::cmp::Reverse(u64::from(kind.pixel_width()) * u64::from(kind.pixel_height()))
    });
    let colour = ColourInfo {
        model: ColourModel::Rgba,
        bits: 8,
    };
    for kind in kinds {
        if let Some(picture) = icon_picture(&family, kind) {
            return Ok((picture, colour));
        }
    }
    Err(decode_error(
        "the icon family holds no picture this build reads",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_family_decodes_to_its_largest_picture() {
        let mut family = IconFamily::new();
        for (kind, side) in [(IconType::RGBA32_16x16, 16), (IconType::RGBA32_32x32, 32)] {
            let picture = image::RgbaImage::from_pixel(side, side, image::Rgba([200; 4]));
            let mut png = Cursor::new(Vec::new());
            picture.write_to(&mut png, image::ImageFormat::Png).unwrap();
            family
                .elements
                .push(icns::IconElement::new(kind.ostype(), png.into_inner()));
        }
        let mut bytes = Vec::new();
        family.write(&mut bytes).unwrap();
        let (picture, _) = icns(&bytes).unwrap();
        assert_eq!(
            picture.size(),
            PixelSize {
                width: PixelLen(32),
                height: PixelLen(32)
            }
        );
    }

    #[test]
    fn garbage_is_not_a_photoshop_file_or_an_icon_family() {
        for (name, result) in [
            ("psd", psd(b"8BPS\0\x01 not really a header").map(|_| ())),
            ("psd short", psd(b"8B").map(|_| ())),
            ("icns", icns(b"icns\0\0\0\x10xxxxxxxx").map(|_| ())),
        ] {
            assert!(result.is_err(), "{name}: {result:?}");
        }
    }
}
