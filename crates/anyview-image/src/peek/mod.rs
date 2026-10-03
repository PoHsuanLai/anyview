//! The light tier for images: a downscaled picture that fits the peek budget, and the facts the
//! pane lists. `RasterPeek` and `VectorPeek` are the two `Peek` implementations, one per kind.

mod facts;

use crate::decode::{ColourInfo, FrameCount, Looked, look, read};
use crate::error::ImageError;
use crate::exif::ExifFacts;
use crate::pixels::Rgba8;
use crate::scale::{Resampling, fit_area, peek_area, resampled};
use anyview_core::{
    Facts, FormatDetail, FormatKind, Peek, PeekBudget, PixelSize, RasterFormat, Sniffed, Source,
};

/// The kind of image a peek looked at, for the facts' first row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeekedFormat {
    /// A raster format.
    Raster(RasterFormat),
    /// SVG.
    Svg,
}

/// What a peek of an image holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePeek {
    /// The first picture, upright, straight alpha, within the budget's pixels and bytes. An
    /// animation shows its first frame.
    pub picture: Rgba8,
    /// The size of the whole image, upright, before it was reduced.
    pub source_size: PixelSize,
    /// How many pictures the file holds; more than one is an animation.
    pub frames: FrameCount,
    /// How the file stores its colour, when the format says.
    pub colour: Option<ColourInfo>,
    /// What the EXIF block says, or nothing.
    pub exif: ExifFacts,
    /// The format.
    pub format: PeekedFormat,
}

/// The peek of every raster image: PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, TGA, QOI, JPEG XL and
/// (with the `avif` feature) AVIF.
#[derive(Debug, Clone, Copy)]
pub struct RasterPeek;

/// The peek of an SVG, drawn at the size the budget allows.
#[derive(Debug, Clone, Copy)]
pub struct VectorPeek;

impl Peek for RasterPeek {
    const KIND: FormatKind = FormatKind::Raster;
    type Peeked = ImagePeek;
    type Error = ImageError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<ImagePeek, ImageError> {
        peek_image(src, sniffed, budget, Self::KIND)
    }

    fn facts(peeked: &ImagePeek) -> Facts {
        facts::of(peeked)
    }
}

impl Peek for VectorPeek {
    const KIND: FormatKind = FormatKind::Vector;
    type Peeked = ImagePeek;
    type Error = ImageError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<ImagePeek, ImageError> {
        peek_image(src, sniffed, budget, Self::KIND)
    }

    fn facts(peeked: &ImagePeek) -> Facts {
        facts::of(peeked)
    }
}

/// Reads the file, decodes its first picture and reduces it to the budget: at most
/// `min(budget.pixels, budget.bytes / 4)` pixels. The budget's time is the caller's to enforce, by
/// abandoning the worker; this crate reads no clock.
fn peek_image(
    src: &Source,
    sniffed: &Sniffed,
    budget: &PeekBudget,
    expected: FormatKind,
) -> Result<ImagePeek, ImageError> {
    if sniffed.kind() != expected {
        return Err(ImageError::WrongKind {
            kind: sniffed.kind(),
        });
    }
    let area = peek_area(budget)?;
    let bytes = read(src)?;
    let looked = look(&bytes, sniffed, area)?;
    Ok(reduced(looked, sniffed, area))
}

fn reduced(looked: Looked, sniffed: &Sniffed, area: anyview_core::PixelArea) -> ImagePeek {
    let fitted = fit_area(looked.picture.size(), area);
    let picture = resampled(&looked.picture, fitted, Resampling::Fast);
    // The codec was chosen from this same detail, so anything but a raster format is the SVG one.
    let format = if let FormatDetail::Raster(format) = sniffed.detail() {
        PeekedFormat::Raster(*format)
    } else {
        PeekedFormat::Svg
    };
    ImagePeek {
        picture,
        source_size: looked.source_size,
        frames: looked.frames,
        colour: looked.colour,
        exif: looked.exif,
        format,
    }
}
