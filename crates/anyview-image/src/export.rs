//! Raster exports: the jobs a choice becomes, and the file one of those jobs writes.

use crate::decode::{Decoded, Svg, declared_size_of, decode_bytes};
use crate::encode::encode_with_metadata;
use crate::error::ImageError;
use crate::exif::ExifFacts;
use crate::orientation::ExifOrientation;
use crate::pixels::Rgba8;
use crate::scale::resized;
use anyview_core::{
    ExportJob, FileHead, FilePath, FormatDetail, FormatKind, MetadataCarry, NonEmpty, PdfPages,
    PixelSize, PixelSource, RasterExport, RasterFormat, RasterTarget, Resize, SniffStep, Sniffed,
    sniff,
};

/// The jobs that write `choice` for the image `file`. A re-encode keeps the original's EXIF and
/// ICC profile (a vector image has none to keep; its encoder drops nothing it holds); a PDF puts
/// the image on one page.
pub fn plan_export(file: &FilePath, choice: RasterExport) -> Vec<ExportJob> {
    match choice {
        RasterExport::Image(target, resize) => vec![ExportJob::EncodeRaster {
            pixels: PixelSource::Image {
                file: file.clone(),
                resize,
            },
            target,
            keep: MetadataCarry::Keep,
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
        let head = FileHead::new(&bytes[..bytes.len().min(4096)]);
        match sniff(&head, &name) {
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
            Decoded::Still(picture) => Ok(picture),
            Decoded::Animated(animation) => Ok(animation.frames.first().pixels.clone()),
        }
    }
}

/// The bytes of the image `file` resized and encoded as `target`. A vector image carries no
/// metadata, whatever `keep` says. Blocking, and slow for AVIF at a large size.
pub fn encode_file(
    file: &FilePath,
    resize: Resize,
    target: RasterTarget,
    keep: MetadataCarry,
) -> Result<Vec<u8>, ImageError> {
    let original = ImageFile::read(file)?;
    let picture = resized(&original.picture()?, resize);
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
            plan_export(&file, RasterExport::Image(RasterTarget::Tiff, resize)),
            [ExportJob::EncodeRaster {
                pixels: PixelSource::Image {
                    file: file.clone(),
                    resize
                },
                target: RasterTarget::Tiff,
                keep: MetadataCarry::Keep,
            }]
        );
        let jpeg = RasterTarget::Jpeg(Quality::clamped(anyview_core::Percent(70)));
        let [ExportJob::EncodeRaster { target, .. }] =
            plan_export(&file, RasterExport::Image(jpeg, Resize::Original))[..]
        else {
            panic!("one encode")
        };
        assert_eq!(target, jpeg, "the options travel with the target");
        assert_eq!(
            plan_export(&file, RasterExport::Pdf),
            [ExportJob::WritePdf {
                pages: PdfPages::Images(NonEmpty::new(file, Vec::new()))
            }]
        );
    }
}
