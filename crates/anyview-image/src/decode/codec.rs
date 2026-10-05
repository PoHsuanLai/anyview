//! Which decoder a sniffed file goes to: the one match on `RasterFormat` in this crate.

use crate::error::ImageError;
use anyview_core::{FormatDetail, FormatKind, RasterFormat, Sniffed};
use image::ImageFormat;

/// The decoder for one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Codec {
    /// A format the `image` crate decodes.
    Image(ImageFormat),
    /// OpenEXR or Radiance HDR: floating point pictures that are tone mapped to 8 bits.
    HighRange(ImageFormat),
    /// A Photoshop document's flattened composite.
    Psd,
    /// An Apple icon family's largest picture.
    Icns,
    /// JPEG XL, through `jxl-oxide`.
    Jxl,
    /// SVG, through `resvg`.
    Svg,
    /// A camera raw file, shown from the JPEG preview inside it.
    RawPreview,
}

/// The decoder for what `sniffed` says the file is, or why there is none.
pub(crate) fn codec_for(sniffed: &Sniffed) -> Result<Codec, ImageError> {
    match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Vector, _) => Ok(Codec::Svg),
        (FormatKind::Raster, FormatDetail::Raster(format)) => raster_codec(*format),
        (kind, _) => Err(ImageError::WrongKind { kind }),
    }
}

fn raster_codec(format: RasterFormat) -> Result<Codec, ImageError> {
    match format {
        RasterFormat::Png => Ok(Codec::Image(ImageFormat::Png)),
        RasterFormat::Jpeg => Ok(Codec::Image(ImageFormat::Jpeg)),
        RasterFormat::Gif => Ok(Codec::Image(ImageFormat::Gif)),
        RasterFormat::Webp => Ok(Codec::Image(ImageFormat::WebP)),
        RasterFormat::Bmp => Ok(Codec::Image(ImageFormat::Bmp)),
        RasterFormat::Tiff => Ok(Codec::Image(ImageFormat::Tiff)),
        RasterFormat::Ico => Ok(Codec::Image(ImageFormat::Ico)),
        RasterFormat::Tga => Ok(Codec::Image(ImageFormat::Tga)),
        RasterFormat::Qoi => Ok(Codec::Image(ImageFormat::Qoi)),
        RasterFormat::Avif => avif(),
        RasterFormat::Jxl => Ok(Codec::Jxl),
        RasterFormat::Psd => Ok(Codec::Psd),
        RasterFormat::Icns => Ok(Codec::Icns),
        RasterFormat::Exr => Ok(Codec::HighRange(ImageFormat::OpenExr)),
        RasterFormat::Hdr => Ok(Codec::HighRange(ImageFormat::Hdr)),
        RasterFormat::Raw => Ok(Codec::RawPreview),
        RasterFormat::Heic => Err(ImageError::Unsupported { format }),
    }
}

#[cfg(feature = "avif")]
fn avif() -> Result<Codec, ImageError> {
    Ok(Codec::Image(ImageFormat::Avif))
}

#[cfg(not(feature = "avif"))]
fn avif() -> Result<Codec, ImageError> {
    Err(ImageError::NotCompiledIn {
        format: RasterFormat::Avif,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FileHead, FileName, SniffStep, sniff};

    fn sniffed(bytes: &[u8], name: &str) -> Sniffed {
        let name = FileName::new(name).unwrap();
        match sniff(&FileHead::new(bytes), &name) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(_) => panic!("not a zip"),
        }
    }

    #[test]
    fn each_sniffed_file_goes_to_its_decoder_or_is_refused_for_a_reason() {
        let png = include_bytes!("../../tests/fixtures/quadrants.png");
        let svg = include_bytes!("../../tests/fixtures/logo.svg");
        let jxl = include_bytes!("../../tests/fixtures/photo.jxl");
        let avif = include_bytes!("../../tests/fixtures/photo.avif");
        // name, bytes, file name, result
        type Case<'a> = (&'a str, &'a [u8], &'a str, Result<Codec, ImageError>);
        let cases: &[Case] = &[
            ("png", png, "a.png", Ok(Codec::Image(ImageFormat::Png))),
            ("svg", svg, "a.svg", Ok(Codec::Svg)),
            ("jxl", jxl, "a.jxl", Ok(Codec::Jxl)),
            (
                "text",
                b"hello",
                "a.txt",
                Err(ImageError::WrongKind {
                    kind: FormatKind::PlainText,
                }),
            ),
            (
                "avif",
                avif,
                "a.avif",
                if cfg!(feature = "avif") {
                    Ok(Codec::Image(ImageFormat::Avif))
                } else {
                    Err(ImageError::NotCompiledIn {
                        format: RasterFormat::Avif,
                    })
                },
            ),
        ];
        for (name, bytes, file, want) in cases {
            assert_eq!(&codec_for(&sniffed(bytes, file)), want, "{name}");
        }
    }

    #[test]
    fn formats_without_a_decoder_are_unsupported() {
        for format in [RasterFormat::Heic] {
            assert_eq!(
                raster_codec(format),
                Err(ImageError::Unsupported { format })
            );
        }
    }
}
