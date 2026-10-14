//! Raster exports: the jobs a choice becomes, and the file one of those jobs writes.

use crate::decode::{Decoded, Svg, declared_size_of, decode_bytes};
use crate::edit::adjusted_pixels;
use crate::encode::{encode, encode_with_metadata};
use crate::error::ImageError;
use crate::exif::ExifFacts;
use crate::orientation::ExifOrientation;
use crate::pixels::Rgba8;
use crate::scale::resized;
use anyview_core::{
    Adjust, ExportJob, FileHead, FileName, FilePath, FormatDetail, FormatKind, MetadataCarry,
    NonEmpty, PdfPages, PixelSize, PixelSource, RasterExport, RasterFormat, RasterTarget, Resize,
    SniffStep, Sniffed, sniff,
};

/// The jobs that write `choice` for the image `file`. A re-encode keeps the original's EXIF and
/// ICC profile unless the choice drops them (a vector image has none to keep); one of an
/// adjusted picture has the person's turns, mirrors and cuts in its pixels; a PDF puts the image
/// on one page.
#[must_use]
pub fn plan_export(file: &FilePath, choice: RasterExport) -> Vec<ExportJob> {
    match choice {
        RasterExport::Image(target, resize, keep) => vec![ExportJob::EncodeRaster {
            pixels: PixelSource::Image {
                file: file.clone(),
                adjust: Adjust::NONE,
                resize,
            },
            target,
            keep,
        }],
        RasterExport::Adjusted(target, adjust) => vec![ExportJob::EncodeRaster {
            pixels: PixelSource::Image {
                file: file.clone(),
                adjust,
                resize: Resize::Original,
            },
            target,
            keep: MetadataCarry::default(),
        }],
        RasterExport::Pdf => vec![ExportJob::WritePdf {
            pages: PdfPages::Images(NonEmpty::new(file.clone(), Vec::new())),
        }],
    }
}

/// An image file as it lies on disk and what sniffing made of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFile {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// What it is.
    pub sniffed: Sniffed,
}

impl ImageFile {
    /// The file at `file`, read whole and sniffed. Blocking.
    pub fn read(file: &FilePath) -> Result<ImageFile, ImageError> {
        let bytes = std::fs::read(file.as_path()).map_err(|error| ImageError::Read {
            path: file.as_path().to_path_buf(),
            kind: error.kind(),
        })?;
        let name = file.file_name().ok_or(ImageError::Read {
            path: file.as_path().to_path_buf(),
            kind: std::io::ErrorKind::InvalidInput,
        })?;
        ImageFile::from_bytes(bytes, &name)
    }

    /// The image whose bytes are `bytes`, called `name` (an entry of a comic, say), sniffed.
    pub fn from_bytes(bytes: Vec<u8>, name: &FileName) -> Result<ImageFile, ImageError> {
        let head = FileHead::new(&bytes);
        match sniff(&head, name) {
            SniffStep::Done(sniffed) => Ok(ImageFile { bytes, sniffed }),
            SniffStep::LookInside(_) => Err(ImageError::WrongKind {
                kind: FormatKind::Archive,
            }),
        }
    }

    /// The size an SVG declares, or `None` for a file that is not one.
    pub fn vector_size(&self) -> Result<Option<PixelSize>, ImageError> {
        if self.sniffed.kind() == FormatKind::Vector {
            Svg::parse(&self.bytes).map(|svg| Some(svg.intrinsic()))
        } else {
            Ok(None)
        }
    }

    /// The size of a JPEG that is already upright, which a PDF can hold as the file is; `None`
    /// for anything else (a PDF reads no EXIF orientation, so a turned one is drawn instead).
    pub fn upright_jpeg_size(&self) -> Result<Option<PixelSize>, ImageError> {
        let is_jpeg = self.sniffed.detail() == &FormatDetail::Raster(RasterFormat::Jpeg);
        if !is_jpeg || ExifFacts::read(&self.bytes).orientation != ExifOrientation::UPRIGHT {
            return Ok(None);
        }
        declared_size_of(&self.bytes, &self.sniffed)
    }

    /// The upright picture of a still image. An animation is exported as its first frame.
    pub fn picture(&self) -> Result<Rgba8, ImageError> {
        match decode_bytes(&self.bytes, &self.sniffed)? {
            Decoded::Still(picture) | Decoded::HeldStill { picture, .. } => Ok(picture),
            Decoded::Animated(animation) => Ok(animation.frames.first().pixels.clone()),
        }
    }
}

/// The bytes of the image `file` with `adjust` applied to its pixels, resized and encoded as
/// `target`. A vector image carries no metadata, whatever `keep` says. Blocking, and slow for
/// AVIF at a large size.
pub fn encode_file(
    file: &FilePath,
    adjust: Adjust,
    resize: Resize,
    target: RasterTarget,
    keep: MetadataCarry,
) -> Result<Vec<u8>, ImageError> {
    encode_file_via(file, adjust, resize, target, keep, |_| None)
}

/// [`encode_file`] for a file whose pixels a plugin may decode: `plugin` is asked for the upright
/// picture of a camera raw file first (the viewer shows the plugin's development, not the preview
/// inside the file) and of a file the built-in decoders have no decoder for. A picture a plugin
/// made is written with no metadata: its pixels are not in the file's colour space, and what the
/// file says of orientation and place was not read by anyone here.
pub fn encode_file_via(
    file: &FilePath,
    adjust: Adjust,
    resize: Resize,
    target: RasterTarget,
    keep: MetadataCarry,
    plugin: impl Fn(&Sniffed) -> Option<Rgba8>,
) -> Result<Vec<u8>, ImageError> {
    let original = ImageFile::read(file)?;
    let developed = if original.sniffed.detail() == &FormatDetail::Raster(RasterFormat::Raw) {
        plugin(&original.sniffed)
    } else {
        None
    };
    let (upright, by_plugin) = match developed {
        Some(picture) => (picture, true),
        None => match original.picture() {
            Ok(picture) => (picture, false),
            Err(error @ (ImageError::Unsupported { .. } | ImageError::NotCompiledIn { .. })) => {
                match plugin(&original.sniffed) {
                    Some(picture) => (picture, true),
                    None => return Err(error),
                }
            }
            Err(error) => return Err(error),
        },
    };
    let picture = if adjust.is_none() {
        resized(&upright, resize)
    } else {
        resized(&adjusted_pixels(&upright, adjust), resize)
    };
    if by_plugin {
        return encode(&picture, target);
    }
    let keep = if original.sniffed.kind() == FormatKind::Vector {
        MetadataCarry::Drop
    } else {
        keep
    };
    encode_with_metadata(&picture, target, &original.bytes, keep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{PixelLen, Quality};

    #[test]
    fn an_image_choice_is_one_encode_that_keeps_metadata_or_one_pdf_page() {
        let file = FilePath::new("/pics/photo.jpg").unwrap();
        let resize = Resize::LongEdge(PixelLen(800));
        assert_eq!(
            plan_export(
                &file,
                RasterExport::Image(RasterTarget::Tiff, resize, MetadataCarry::Keep)
            ),
            [ExportJob::EncodeRaster {
                pixels: PixelSource::Image {
                    file: file.clone(),
                    adjust: Adjust::NONE,
                    resize
                },
                target: RasterTarget::Tiff,
                keep: MetadataCarry::Keep,
            }]
        );
        let jpeg = RasterTarget::Jpeg(Quality::clamped(anyview_core::Percent(70)));
        let [ExportJob::EncodeRaster { target, keep, .. }] = plan_export(
            &file,
            RasterExport::Image(jpeg, Resize::Original, MetadataCarry::Drop),
        )[..] else {
            panic!("one encode")
        };
        assert_eq!(target, jpeg, "the options travel with the target");
        assert_eq!(
            keep,
            MetadataCarry::Drop,
            "so does leaving the metadata out"
        );
        assert_eq!(
            plan_export(&file, RasterExport::Pdf),
            [ExportJob::WritePdf {
                pages: PdfPages::Images(NonEmpty::new(file, Vec::new()))
            }]
        );
    }
}
