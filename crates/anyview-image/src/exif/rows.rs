//! The rows of a photo's EXIF block for the viewer's Info panel. `ExifFacts` holds no place, so
//! no preview can list one (see `Location`), and none lists a serial number, because none is
//! read.

use super::{ExifFacts, Flash, format};
use crate::resolution::Resolution;
use anyview_core::{FactLabel, FactTime, FactValue, FactZone, Facts, LocalZone};

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

    /// The capture time as a date value in the person's own zone: `1 May 2024 at 12:30`. A time
    /// with no offset is the person's own already and is shown as written. `None` when the block
    /// has none or it is not a time EXIF would write (cameras without a clock write `0000:00:00
    /// 00:00:00` or blanks).
    pub(crate) fn taken_value(&self) -> Option<FactValue> {
        self.taken_value_in(&LocalZone::system())
    }

    fn taken_value_in(&self, zone: &LocalZone) -> Option<FactValue> {
        let when = FactTime::parse_exif(self.taken.as_deref()?)?;
        let when = match self.taken_offset {
            Some(minutes) => when.in_zone(FactZone::Offset(minutes)),
            None => when,
        };
        Some(FactValue::date_in(when, zone))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(raw: &str, offset: Option<i16>) -> ExifFacts {
        ExifFacts {
            taken: Some(raw.to_owned()),
            taken_offset: offset,
            ..ExifFacts::none()
        }
    }

    #[test]
    fn capture_times_without_an_offset_are_shown_as_written() {
        let zone = LocalZone::fixed(-480);
        let shown = |raw: &str, offset| {
            taken(raw, offset)
                .taken_value_in(&zone)
                .map(|value| value.as_str().to_owned())
        };
        assert_eq!(
            shown("2024:05:01 12:30:45", None).as_deref(),
            Some("1 May 2024 at 12:30")
        );
        assert_eq!(
            shown("2024:05:01 12:30", None).as_deref(),
            Some("1 May 2024 at 12:30")
        );
        // An offset tag converts: 12:30 at +02:00 is 02:30 at -08:00.
        assert_eq!(
            shown("2024:05:01 12:30:45", Some(120)).as_deref(),
            Some("1 May 2024 at 02:30")
        );
        assert_eq!(
            shown("2024:05:01 03:30:45", Some(-480)).as_deref(),
            Some("1 May 2024 at 03:30")
        );
    }

    #[test]
    fn capture_times_that_are_not_times_are_dropped() {
        for bad in [
            "0000:00:00 00:00:00",
            "    :  :     :  :  ",
            "",
            "   ",
            "2024-05-01 12:30:45",
            "2024:05:01",
            "yesterday",
            "2024:13:01 12:00:00",
        ] {
            let facts = taken(bad, None);
            assert_eq!(facts.taken_value(), None, "{bad:?}");
            assert!(
                facts
                    .camera_facts()
                    .rows()
                    .iter()
                    .all(|row| row.label != FactLabel::Taken),
                "{bad:?}"
            );
        }
    }
}
