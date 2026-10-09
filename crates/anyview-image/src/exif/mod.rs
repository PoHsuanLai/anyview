//! What a photo's EXIF block says: orientation for decoding, camera, lens, exposure and date for
//! the facts. Reading goes through `kamadak-exif`; patching the orientation is `patch`.

mod flash;
mod format;
mod location;
mod patch;
mod rows;

#[cfg(test)]
mod tests;

pub use flash::{Flash, FlashMode, FlashState};
pub use location::Location;
pub(crate) use patch::with_orientation;

use crate::orientation::ExifOrientation;
use crate::resolution::Resolution;
use exif::{In, Reader, Tag, Value};
use std::io::Cursor;

/// A positive fraction as EXIF stores it. Kept as two integers so the facts stay `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    /// The numerator.
    pub numerator: u32,
    /// The denominator, never zero.
    pub denominator: u32,
}

/// The shutter, aperture, sensitivity and focal length a photo was taken with. Each is absent when
/// the camera did not record it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Exposure {
    /// The shutter time in seconds.
    pub shutter: Option<Ratio>,
    /// The f-number.
    pub aperture: Option<Ratio>,
    /// The ISO sensitivity.
    pub iso: Option<u32>,
    /// The focal length in millimetres.
    pub focal_length: Option<Ratio>,
}

/// A signed fraction as EXIF stores it, such as an exposure bias in stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignedRatio {
    /// The numerator.
    pub numerator: i32,
    /// The denominator, never zero.
    pub denominator: i32,
}

/// The facts of a photo's EXIF block. An image with no EXIF block, or none the reader
/// understands, has [`ExifFacts::none`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExifFacts {
    /// How the stored pixels are placed to display upright.
    pub orientation: ExifOrientation,
    /// The camera maker.
    pub make: Option<String>,
    /// The camera model.
    pub model: Option<String>,
    /// The lens model.
    pub lens: Option<String>,
    /// How the photo was exposed.
    pub exposure: Exposure,
    /// The capture time as EXIF writes it, `YYYY:MM:DD HH:MM:SS`.
    pub taken: Option<String>,
    /// How far the exposure was moved from the metered one, in stops.
    pub bias: Option<SignedRatio>,
    /// Whether and how the flash fired.
    pub flash: Option<Flash>,
    /// The program that made or last saved the file.
    pub software: Option<String>,
    /// The copyright notice.
    pub copyright: Option<String>,
    /// The density the picture is meant to be shown at.
    pub resolution: Option<Resolution>,
    /// Where the photo was taken. Never listed by a preview: see [`ExifFacts::location_facts`].
    /// The body and lens serial numbers the block may hold are not read at all.
    pub location: Option<Location>,
}

impl ExifFacts {
    /// The facts of an image that has no EXIF: upright, nothing known.
    pub fn none() -> Self {
        ExifFacts {
            orientation: ExifOrientation::UPRIGHT,
            make: None,
            model: None,
            lens: None,
            exposure: Exposure::default(),
            taken: None,
            bias: None,
            flash: None,
            software: None,
            copyright: None,
            resolution: None,
            location: None,
        }
    }

    /// The facts in the EXIF block of an image file (JPEG, PNG, WebP, TIFF or HEIF container), or
    /// [`ExifFacts::none`] when it has no readable block.
    pub fn read(file: &[u8]) -> Self {
        let Ok(exif) = Reader::new().read_from_container(&mut Cursor::new(file)) else {
            return ExifFacts::none();
        };
        let field = |tag| exif.get_field(tag, In::PRIMARY).map(|f| &f.value);
        let orientation = field(Tag::Orientation)
            .and_then(|value| value.get_uint(0))
            .and_then(|tag| u16::try_from(tag).ok())
            .and_then(ExifOrientation::from_tag)
            .unwrap_or(ExifOrientation::UPRIGHT);
        ExifFacts {
            orientation,
            make: field(Tag::Make).and_then(text),
            model: field(Tag::Model).and_then(text),
            lens: field(Tag::LensModel).and_then(text),
            exposure: Exposure {
                shutter: field(Tag::ExposureTime).and_then(ratio),
                aperture: field(Tag::FNumber).and_then(ratio),
                iso: field(Tag::PhotographicSensitivity).and_then(|v| v.get_uint(0)),
                focal_length: field(Tag::FocalLength).and_then(ratio),
            },
            taken: field(Tag::DateTimeOriginal)
                .or_else(|| field(Tag::DateTime))
                .and_then(text),
            bias: field(Tag::ExposureBiasValue).and_then(signed_ratio),
            flash: field(Tag::Flash)
                .and_then(|value| value.get_uint(0))
                .map(Flash::of_field),
            software: field(Tag::Software).and_then(text),
            copyright: field(Tag::Copyright).and_then(text),
            resolution: resolution(&exif),
            location: Location::read(&exif),
        }
    }

    /// Whether the EXIF block of an image file says how it is oriented at all (as opposed to
    /// saying nothing, which reads as upright).
    pub(crate) fn has_orientation(file: &[u8]) -> bool {
        Reader::new()
            .read_from_container(&mut Cursor::new(file))
            .is_ok_and(|exif| {
                exif.get_field(Tag::Orientation, In::PRIMARY)
                    .and_then(|field| field.value.get_uint(0))
                    .and_then(|tag| u16::try_from(tag).ok())
                    .and_then(ExifOrientation::from_tag)
                    .is_some()
            })
    }

    /// `Canon EOS R5`: the model, with the maker in front unless the model already starts with it.
    pub fn camera(&self) -> Option<String> {
        format::camera(self.make.as_deref(), self.model.as_deref())
    }

    /// `1/200 s · f/2.8 · ISO 100 · 35 mm`, the parts that were recorded.
    pub fn exposure_text(&self) -> Option<String> {
        format::exposure(&self.exposure)
    }

    /// `2024-05-01 12:30`, or the raw text when it is not a date EXIF would write.
    pub fn taken_text(&self) -> Option<String> {
        self.taken.as_deref().map(format::taken)
    }
}

/// The first string of an ASCII value, with the padding cameras add trimmed; `None` when empty.
fn text(value: &Value) -> Option<String> {
    let Value::Ascii(strings) = value else {
        return None;
    };
    let first = strings.first()?;
    let text = String::from_utf8_lossy(first);
    let trimmed = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The first fraction of a rational value; `None` when the denominator is zero.
fn ratio(value: &Value) -> Option<Ratio> {
    let Value::Rational(rationals) = value else {
        return None;
    };
    let first = rationals.first()?;
    (first.denom != 0).then_some(Ratio {
        numerator: first.num,
        denominator: first.denom,
    })
}

/// The first signed fraction of a value; `None` when the denominator is zero.
fn signed_ratio(value: &Value) -> Option<SignedRatio> {
    let Value::SRational(rationals) = value else {
        return None;
    };
    let first = rationals.first()?;
    (first.denom != 0).then_some(SignedRatio {
        numerator: first.num,
        denominator: first.denom,
    })
}

/// The dots an inch the block says the picture has, from its X and Y resolution and their unit
/// (inches unless it says centimetres).
fn resolution(exif: &exif::Exif) -> Option<Resolution> {
    let field = |tag| exif.get_field(tag, In::PRIMARY).map(|f| &f.value);
    let per_unit = |tag| field(tag).and_then(ratio);
    let centimetres = field(Tag::ResolutionUnit).and_then(|unit| unit.get_uint(0)) == Some(3);
    Resolution::per_unit(
        per_unit(Tag::XResolution)?,
        per_unit(Tag::YResolution)?,
        if centimetres {
            crate::resolution::Unit::Centimetre
        } else {
            crate::resolution::Unit::Inch
        },
    )
}
