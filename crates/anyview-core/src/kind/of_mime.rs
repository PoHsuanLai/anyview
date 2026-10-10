//! The kind a media type names, for callers that have a MIME type from an index and no bytes to
//! sniff (the launcher's file rows). The families state their own types; the rest is a few text
//! families.

use super::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, FormatKind, MediaContainer, Mime,
    OfficeFormat, RasterFormat, TreeFormat, from_mime,
};

/// Text types that are source or configuration: highlighted, not plain.
const CODE: &[&str] = &[
    "text/html",
    "text/css",
    "text/javascript",
    "application/xml",
    "application/toml",
    "application/x-toml",
    "application/yaml",
    "application/x-yaml",
    "application/javascript",
    "application/x-shellscript",
    "application/x-sh",
    "application/sql",
    "application/x-desktop",
    "application/x-perl",
    "application/x-python",
    "application/x-ruby",
];

/// What a file of type `mime` is, as far as the type says: a family's own types name their kind,
/// `image/*`, `video/*`, `audio/*` and `font/*` name theirs, and text is plain unless it is
/// source. A type that names nothing the viewer knows is [`FormatKind::Other`]. The bytes can
/// say more (`sniff` is the authority once a file is open); this is for a row that has none.
#[must_use]
pub fn kind_of_mime(mime: &Mime) -> FormatKind {
    let text = mime.as_str();
    if let Some(kind) = exact(text) {
        return kind;
    }
    let (family, subtype) = text.split_once('/').unwrap_or((text, ""));
    match family {
        "image" => FormatKind::Raster,
        "video" => FormatKind::Video,
        "audio" => FormatKind::Audio,
        "font" => FormatKind::Font,
        "text" if subtype == "plain" => FormatKind::PlainText,
        "text" => FormatKind::Code,
        "application" if CODE.contains(&text) => FormatKind::Code,
        _ => FormatKind::Other,
    }
}

/// The kinds whose types are listed one by one: the single types and each family's.
fn exact(text: &str) -> Option<FormatKind> {
    match text {
        "application/pdf" => return Some(FormatKind::Pdf),
        "image/svg+xml" => return Some(FormatKind::Vector),
        "text/markdown" => return Some(FormatKind::Markdown),
        "inode/directory" => return Some(FormatKind::Folder),
        _ => {}
    }
    from_mime::<RasterFormat>(text)
        .map(|_| FormatKind::Raster)
        .or_else(|| from_mime::<MediaContainer>(text).map(MediaContainer::kind))
        .or_else(|| from_mime::<Delimiter>(text).map(|_| FormatKind::Table))
        .or_else(|| from_mime::<TreeFormat>(text).map(|_| FormatKind::Tree))
        .or_else(|| from_mime::<FontFormat>(text).map(|_| FormatKind::Font))
        .or_else(|| from_mime::<ArchiveFormat>(text).map(|_| FormatKind::Archive))
        .or_else(|| from_mime::<BookFormat>(text).map(|_| FormatKind::Book))
        .or_else(|| from_mime::<OfficeFormat>(text).map(OfficeFormat::kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_media_type_names_the_kind_it_belongs_to() {
        // media type, kind
        const CASES: &[(&str, FormatKind)] = &[
            ("application/pdf", FormatKind::Pdf),
            ("image/png", FormatKind::Raster),
            ("image/x-xcf", FormatKind::Raster),
            ("image/svg+xml", FormatKind::Vector),
            ("video/mp4", FormatKind::Video),
            ("video/x-unknown", FormatKind::Video),
            ("audio/flac", FormatKind::Audio),
            ("audio/x-opus+ogg", FormatKind::Audio),
            ("text/markdown", FormatKind::Markdown),
            ("text/plain", FormatKind::PlainText),
            ("text/x-rust", FormatKind::Code),
            ("text/html", FormatKind::Code),
            ("application/x-shellscript", FormatKind::Code),
            ("text/csv", FormatKind::Table),
            ("text/tab-separated-values", FormatKind::Table),
            ("application/json", FormatKind::Tree),
            ("application/x-ndjson", FormatKind::Tree),
            ("font/ttf", FormatKind::Font),
            ("application/zip", FormatKind::Archive),
            ("application/x-7z-compressed", FormatKind::Archive),
            ("application/epub+zip", FormatKind::Book),
            (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                FormatKind::Office,
            ),
            (
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                FormatKind::Table,
            ),
            (
                "application/vnd.oasis.opendocument.spreadsheet",
                FormatKind::Table,
            ),
            ("application/vnd.ms-excel", FormatKind::Table),
            ("inode/directory", FormatKind::Folder),
            ("application/octet-stream", FormatKind::Other),
            ("application/x-unheard-of", FormatKind::Other),
        ];
        for (text, want) in CASES {
            let mime = Mime::parse(text).unwrap();
            assert_eq!(kind_of_mime(&mime), *want, "{text}");
        }
    }
}
