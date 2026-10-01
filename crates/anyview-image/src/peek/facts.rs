//! The rows a pane lists about an image.

use super::{ImagePeek, PeekedFormat};
use anyview_core::{FactLabel, FactValue, Facts};
use ds_core::word::Word;

/// `PNG image`, `SVG image`.
fn kind_text(format: PeekedFormat) -> String {
    let name = match format {
        PeekedFormat::Raster(raster) => raster.slug().to_ascii_uppercase(),
        PeekedFormat::Svg => "SVG".to_owned(),
    };
    format!("{name} image")
}

/// Kind, dimensions, frame count for an animation, colour, then whatever EXIF recorded.
pub(super) fn of(peeked: &ImagePeek) -> Facts {
    let text = |label: FactLabel, value: Option<String>, facts: Facts| match value {
        Some(value) => facts.with(label, FactValue::text(value)),
        None => facts,
    };
    let mut facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(kind_text(peeked.format)))
        .with(
            FactLabel::Dimensions,
            FactValue::dimensions(peeked.source_size),
        );
    if peeked.frames.0 > 1 {
        facts = facts.with(
            FactLabel::Frames,
            FactValue::text(peeked.frames.0.to_string()),
        );
    }
    facts = text(FactLabel::Colour, peeked.colour.map(|c| c.text()), facts);
    facts = text(FactLabel::Camera, peeked.exif.camera(), facts);
    facts = text(FactLabel::Lens, peeked.exif.lens.clone(), facts);
    facts = text(FactLabel::Exposure, peeked.exif.exposure_text(), facts);
    text(FactLabel::Taken, peeked.exif.taken_text(), facts)
}
