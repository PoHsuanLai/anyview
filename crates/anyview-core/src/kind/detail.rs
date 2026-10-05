//! `FormatDetail`: what a kind of file is more precisely.

use super::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, MediaContainer, OfficeFormat, RasterFormat,
    SyntaxName, TextEncoding, TreeFormat,
};

/// The format inside a [`FormatKind`](super::FormatKind): which raster format, which syntax,
/// which delimiter. Each variant belongs to the kinds named in its doc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FormatDetail {
    /// Nothing more to say: PDF, SVG, Markdown, a folder, a file of unknown type.
    None,
    /// The kind `Raster`.
    Raster(RasterFormat),
    /// The kind `Code`: the highlighter's syntax.
    Code(SyntaxName),
    /// The kind `Table`, a delimited file.
    Table(Delimiter),
    /// The kind `Tree`.
    Tree(TreeFormat),
    /// The kind `PlainText`: how its bytes are encoded.
    Text(TextEncoding),
    /// The kinds `Video` and `Audio`.
    Media(MediaContainer),
    /// The kind `Font`.
    Font(FontFormat),
    /// The kind `Archive`.
    Archive(ArchiveFormat),
    /// The kind `Book`.
    Book(BookFormat),
    /// The kind `Office`, and the kind `Table` for a spreadsheet (`OfficeFormat::kind`).
    Office(OfficeFormat),
}
