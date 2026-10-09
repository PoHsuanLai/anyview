//! Font file formats.

use super::family::Family;
use ds_core::word::Word;

/// A font file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum FontFormat {
    /// TrueType.
    Ttf,
    /// OpenType.
    Otf,
    /// A TrueType or OpenType collection.
    Ttc,
    /// WOFF.
    Woff,
    /// WOFF2.
    Woff2,
}

impl Family for FontFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            FontFormat::Ttf => &["ttf"],
            FontFormat::Otf => &["otf"],
            FontFormat::Ttc => &["ttc", "otc"],
            FontFormat::Woff => &["woff"],
            FontFormat::Woff2 => &["woff2"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            FontFormat::Ttf => "font/ttf",
            FontFormat::Otf => "font/otf",
            FontFormat::Ttc => "font/collection",
            FontFormat::Woff => "font/woff",
            FontFormat::Woff2 => "font/woff2",
        }
    }
}
