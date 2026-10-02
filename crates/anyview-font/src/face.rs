//! Reading a font's names, glyph count and variation with skrifa.

use crate::error::FontError;
use crate::specimen::{Specimen, specimen};
use skrifa::raw::{FileRef, TableProvider};
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

/// What a face says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// Its family name, `Inter`; empty when the font names none.
    pub family: String,
    /// Its style name, `Bold Italic`; empty when the font names none.
    pub style: String,
    /// How many glyphs it draws.
    pub glyphs: u32,
    /// Whether it has variation axes (one file, many weights).
    pub variable: Variation,
    /// Sample lines set in it.
    pub specimen: Specimen,
}

/// Whether a face has variation axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variation {
    /// One design per face.
    Fixed,
    /// Axes of weight, width or others.
    Variable,
}

/// The first face of the font file `bytes`, and how many faces the file holds (a collection has
/// several, anything else one).
pub(crate) fn read(bytes: &[u8]) -> Result<(Face, u32), FontError> {
    let file = FileRef::new(bytes).map_err(malformed)?;
    let (font, faces) = match file {
        FileRef::Font(font) => (font, 1),
        FileRef::Collection(collection) => {
            (collection.get(0).map_err(malformed)?, collection.len())
        }
    };
    // A directory that names no `head` or `maxp` is not a font, whatever its header says.
    if font.head().is_err() || font.maxp().is_err() {
        return Err(FontError::Malformed {
            reason: "the font has no head or maxp table".to_owned(),
        });
    }
    Ok((face_of(&font), faces))
}

fn face_of(font: &FontRef<'_>) -> Face {
    let named = |id: StringId| {
        font.localized_strings(id)
            .english_or_first()
            .map(|name| name.to_string())
            .unwrap_or_default()
    };
    let variable = match font.axes().len() {
        0 => Variation::Fixed,
        _ => Variation::Variable,
    };
    Face {
        family: named(StringId::FAMILY_NAME),
        style: named(StringId::SUBFAMILY_NAME),
        glyphs: font
            .metrics(
                skrifa::instance::Size::unscaled(),
                skrifa::instance::LocationRef::default(),
            )
            .glyph_count
            .into(),
        variable,
        specimen: specimen(font),
    }
}

fn malformed(error: impl std::fmt::Display) -> FontError {
    FontError::Malformed {
        reason: error.to_string(),
    }
}
