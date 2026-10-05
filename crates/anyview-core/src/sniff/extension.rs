//! The fallback: a kind from the file's name, used only when its bytes did not say.

use super::Sniffed;
use crate::kind::{
    ArchiveFormat, BookFormat, Delimiter, Family, FontFormat, FormatDetail, FormatKind,
    MediaContainer, Mime, OfficeFormat, RasterFormat, SyntaxName, TextEncoding, TreeFormat,
    from_extension, syntax_for_extension, syntax_for_name,
};
use crate::source::FileName;

/// Extensions of Markdown.
const MARKDOWN: &[&str] = &["md", "markdown", "mdown", "mkd"];

/// The kind of a binary file from its extension: the families that are not text. `None` when the
/// extension names none of them.
pub(super) fn binary(extension: &str) -> Option<Sniffed> {
    if extension.eq_ignore_ascii_case("pdf") {
        return Some(Sniffed::new(
            FormatKind::Pdf,
            Mime::known("application/pdf"),
            FormatDetail::None,
        ));
    }
    if let Some(format) = from_extension::<RasterFormat>(extension) {
        return Some(of(FormatKind::Raster, format, FormatDetail::Raster(format)));
    }
    if let Some(container) = from_extension::<MediaContainer>(extension) {
        return Some(of(
            container.kind(),
            container,
            FormatDetail::Media(container),
        ));
    }
    if let Some(format) = from_extension::<FontFormat>(extension) {
        return Some(of(FormatKind::Font, format, FormatDetail::Font(format)));
    }
    if let Some(format) = from_extension::<ArchiveFormat>(extension) {
        return Some(of(
            FormatKind::Archive,
            format,
            FormatDetail::Archive(format),
        ));
    }
    if let Some(format) = from_extension::<BookFormat>(extension) {
        return Some(of(FormatKind::Book, format, FormatDetail::Book(format)));
    }
    from_extension::<OfficeFormat>(extension)
        .map(|format| of(format.kind(), format, FormatDetail::Office(format)))
}

/// The kind of a text file from its name: Markdown, a table, a tree, an SVG, source code, or plain
/// text with the encoding its bytes had.
pub(super) fn text(name: &FileName, encoding: TextEncoding) -> Sniffed {
    let extension = name.extension();
    let is = |known: &[&str]| {
        extension.is_some_and(|ext| known.iter().any(|k| k.eq_ignore_ascii_case(ext)))
    };
    if is(MARKDOWN) {
        return Sniffed::new(
            FormatKind::Markdown,
            Mime::known("text/markdown"),
            FormatDetail::None,
        );
    }
    if let Some(delimiter) = extension.and_then(from_extension::<Delimiter>) {
        return of(FormatKind::Table, delimiter, FormatDetail::Table(delimiter));
    }
    if let Some(format) = extension.and_then(from_extension::<TreeFormat>) {
        return of(FormatKind::Tree, format, FormatDetail::Tree(format));
    }
    if is(&["svg"]) {
        return Sniffed::new(
            FormatKind::Vector,
            Mime::known("image/svg+xml"),
            FormatDetail::None,
        );
    }
    let syntax = extension
        .and_then(syntax_for_extension)
        .or_else(|| syntax_for_name(name.as_str()));
    match syntax {
        Some(syntax) => {
            let mime = code_mime(&syntax);
            Sniffed::new(FormatKind::Code, mime, FormatDetail::Code(syntax))
        }
        None => Sniffed::new(
            FormatKind::PlainText,
            Mime::known("text/plain"),
            FormatDetail::Text(encoding),
        ),
    }
}

/// The kind of a file that is neither a known binary nor text.
pub(super) fn unknown() -> Sniffed {
    Sniffed::new(FormatKind::Other, Mime::OCTET_STREAM, FormatDetail::None)
}

/// A sniffed file of `kind` in `format`, whose MIME type the family states.
fn of(kind: FormatKind, format: impl Family, detail: FormatDetail) -> Sniffed {
    Sniffed::new(kind, Mime::known(format.mime()), detail)
}

/// The media type of source code: a few syntaxes have their own, the rest are `text/plain`.
fn code_mime(syntax: &SyntaxName) -> Mime {
    match syntax.as_str() {
        "html" => Mime::known("text/html"),
        "css" => Mime::known("text/css"),
        "xml" => Mime::known("application/xml"),
        _ => Mime::known("text/plain"),
    }
}
