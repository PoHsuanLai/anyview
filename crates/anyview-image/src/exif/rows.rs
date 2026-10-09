//! The rows of a photo's EXIF block for the viewer's Info panel. A preview pane lists
//! [`ExifFacts::camera_facts`] at most, and never [`ExifFacts::location_facts`]; neither lists a
//! serial number, because none is read.

use super::{ExifFacts, Flash, format};
use crate::resolution::Resolution;
use anyview_core::{FactLabel, FactTime, FactValue, Facts};

impl ExifFacts {
    /// The camera, lens, settings, flash, capture time and software, and the copyright notice,
    /// each only when the file recorded it. Shutter and aperture share the Exposure row; the
    /// sensitivity and focal length have rows of their own.
    pub fn camera_facts(&self) -> Facts {
        let text = |text: Option<String>| text.map(FactValue::text);
        let rows = [
            (FactLabel::Camera, text(self.camera())),
            (FactLabel::Lens, text(self.lens.clone())),
            (FactLabel::Exposure, text(format::settings(&self.exposure))),
            (FactLabel::ExposureBias, text(self.bias.map(format::bias))),
            (
                FactLabel::Iso,
                text(self.exposure.iso.map(|iso| iso.to_string())),
            ),
            (
                FactLabel::FocalLength,
                text(self.exposure.focal_length.map(format::focal_length)),
            ),
            (FactLabel::Flash, text(self.flash.map(Flash::text))),
            (FactLabel::Taken, self.taken_value()),
            (FactLabel::Software, text(self.software.clone())),
            (FactLabel::Copyright, text(self.copyright.clone())),
            (
                FactLabel::Resolution,
                text(self.resolution.map(Resolution::text)),
            ),
        ];
        rows.into_iter()
            .filter_map(|(label, value)| Some((label, value?)))
            .fold(Facts::empty(), |facts, (label, value)| {
                facts.with(label, value)
            })
    }

    /// Where the photo was taken: coordinates and altitude, as text. Only the viewer's own Info
    /// panel calls this: a preview pane, a list or a thumbnail may be shared or screenshotted,
    /// and must not say where a photo was taken. Nothing here looks the place up.
    pub fn location_facts(&self) -> Facts {
        let Some(location) = self.location else {
            return Facts::empty();
        };
        let facts = Facts::empty().with(
            FactLabel::Coordinates,
            FactValue::coordinate(location.place),
        );
        match location.altitude {
            Some(metres) => facts.with(FactLabel::Altitude, FactValue::altitude(metres)),
            None => facts,
        }
    }

    /// The capture time as a date value: `1 May 2024 at 12:30`, or the raw text when it is not a
    /// time EXIF would write.
    fn taken_value(&self) -> Option<FactValue> {
        let raw = self.taken.as_deref()?;
        Some(FactTime::parse_exif(raw).map_or_else(|| FactValue::text(raw), FactValue::date))
    }
}
