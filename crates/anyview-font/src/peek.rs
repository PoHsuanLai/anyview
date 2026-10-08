//! The peek of a font: its names, glyph count and a specimen.

use crate::error::FontError;
use crate::face::{Face, Variation, read};
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FontFormat, FormatDetail, FormatKind, Input, Peek,
    PeekBudget, Sniffed,
};

/// What a peek of a font holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontPeeked {
    /// What the file was sniffed as.
    pub format: FontFormat,
    /// The first face, `None` for a container this crate cannot open (WOFF2).
    pub face: Option<Face>,
    /// How many faces the file holds.
    pub faces: u32,
}

/// The peek of the kind `Font`.
#[derive(Debug, Clone, Copy)]
pub struct FontPeek;

impl Peek for FontPeek {
    const KIND: FormatKind = FormatKind::Font;
    type Peeked = FontPeeked;
    type Error = FontError;

    fn peek(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<FontPeeked, FontError> {
        let FormatDetail::Font(format) = sniffed.detail() else {
            return Err(FontError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        match format {
            FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc => {
                let bytes = read_whole(src, budget.bytes)?;
                let (face, faces) = read(&bytes)?;
                Ok(FontPeeked {
                    format: *format,
                    face: Some(face),
                    faces,
                })
            }
            FontFormat::Woff => {
                let bytes = crate::woff::to_sfnt(&read_whole(src, budget.bytes)?)?;
                let (face, faces) = read(&bytes)?;
                Ok(FontPeeked {
                    format: *format,
                    face: Some(face),
                    faces,
                })
            }
            FontFormat::Woff2 => Ok(FontPeeked {
                format: *format,
                face: None,
                faces: 1,
            }),
        }
    }

    fn facts(peeked: &FontPeeked) -> Facts {
        let kind = match (peeked.format, peeked.faces) {
            (FontFormat::Ttf, _) => "TrueType font".to_owned(),
            (FontFormat::Otf, _) => "OpenType font".to_owned(),
            (FontFormat::Woff, _) => "WOFF web font".to_owned(),
            (FontFormat::Woff2, _) => "WOFF2 web font".to_owned(),
            (FontFormat::Ttc, 1) => "Font collection, 1 face".to_owned(),
            (FontFormat::Ttc, faces) => format!("Font collection, {faces} faces"),
        };
        let facts = Facts::empty().with(FactLabel::Kind, FactValue::text(kind));
        let Some(face) = &peeked.face else {
            return facts;
        };
        let mut facts = facts;
        if !face.family.is_empty() {
            facts = facts.with(FactLabel::Family, FactValue::text(face.family.clone()));
        }
        if !face.style.is_empty() {
            let style = match face.variable {
                Variation::Fixed => face.style.clone(),
                Variation::Variable => format!("{}, variable", face.style),
            };
            facts = facts.with(FactLabel::Style, FactValue::text(style));
        }
        facts.with(FactLabel::Glyphs, FactValue::text(face.glyphs.to_string()))
    }
}

/// The whole file, which must fit in the budget: a font's tables lie all over it. The length is
/// the larger of the stamp's and the bytes' own, so a source that understates its size is held to
/// the budget as well.
fn read_whole(src: &Input, allowed: ByteLen) -> Result<Vec<u8>, FontError> {
    let len = ByteLen(src.stamp().len.0.max(src.bytes().len().0));
    if len.0 > allowed.0 {
        return Err(FontError::OverBudget { len, allowed });
    }
    src.bytes()
        .read_range(0..allowed.0)
        .map_err(|error| FontError::Read {
            path: src.label(),
            kind: error.kind(),
        })
}
