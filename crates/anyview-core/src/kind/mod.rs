//! What a file is: its kind, its MIME type and the format inside the kind.

mod archive;
mod book;
mod delimiter;
mod detail;
mod encoding;
mod family;
mod font;
mod media;
mod mime;
mod office;
mod raster;
mod syntax;
mod tree;

pub use archive::ArchiveFormat;
pub use book::BookFormat;
pub use delimiter::Delimiter;
pub use detail::FormatDetail;
pub use encoding::TextEncoding;
pub use font::FontFormat;
pub use media::MediaContainer;
pub use mime::Mime;
pub use office::OfficeFormat;
pub use raster::RasterFormat;
pub use syntax::SyntaxName;
pub use tree::TreeFormat;

pub(crate) use family::{Family, from_extension};
pub(crate) use syntax::{for_extension as syntax_for_extension, for_name as syntax_for_name};

use ds_core::word::Word;

/// What a file is. A closed set, so every decision about files is a match on it; the one place
/// that decides per kind is `crate::profile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum FormatKind {
    /// A PDF document.
    Pdf,
    /// A raster image.
    Raster,
    /// A vector image (SVG).
    Vector,
    /// A video.
    Video,
    /// An audio recording.
    Audio,
    /// A Markdown document.
    Markdown,
    /// Source code or configuration, highlighted.
    Code,
    /// Text with no more structure than lines.
    PlainText,
    /// A delimited table (CSV, TSV).
    Table,
    /// Structured data shown as a tree (JSON).
    Tree,
    /// A font file.
    Font,
    /// An archive whose entries are listed.
    Archive,
    /// A book or comic read page by page.
    Book,
    /// An office or iWork document, shown as facts and opened elsewhere.
    Office,
    /// A folder.
    Folder,
    /// A file of a type the viewer has no part for.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ds_core::testing::word_matches_serde;

    #[test]
    fn kinds_are_stored_as_their_slugs() {
        word_matches_serde::<FormatKind>();
        let json = serde_json::to_string(&FormatKind::PlainText).unwrap();
        assert_eq!(json, "\"plain_text\"");
        assert_eq!(
            serde_json::from_str::<FormatKind>(&json).unwrap(),
            FormatKind::PlainText
        );
    }
}
