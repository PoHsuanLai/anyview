//! The rows the viewer's Info panel lists about a picture beyond its size: colour, density, and
//! what the EXIF block says of the camera and of the place.

use crate::decode::colour_of;
use crate::exif::ExifFacts;
use crate::resolution::of_header;
use anyview_core::{FactLabel, FactValue, Facts, Sniffed};

/// The colour model and depth, the density, the camera rows and the location of the picture in
/// `bytes`, each only when the file records it.
///
/// This is for the viewer's own Info panel: it includes the Location section. A preview pane
/// calls [`ExifFacts::camera_facts`] or the peek's facts instead, which never carry a place.
pub fn picture_facts(bytes: &[u8], sniffed: &Sniffed) -> Facts {
    let exif = ExifFacts::read(bytes);
    let mut facts = Facts::empty();
    if let Some(colour) = colour_of(bytes, sniffed) {
        facts = facts.with(FactLabel::Colour, FactValue::text(colour.text()));
    }
    let camera = exif.camera_facts();
    if exif.resolution.is_none()
        && let Some(resolution) = of_header(bytes, sniffed)
    {
        facts = facts.with(FactLabel::Resolution, FactValue::text(resolution.text()));
    }
    camera
        .rows()
        .iter()
        .fold(facts, |facts, row| facts.with_fact(row.clone()))
        .then(exif.location_facts())
}
