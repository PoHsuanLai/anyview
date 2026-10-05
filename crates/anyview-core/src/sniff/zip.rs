//! The second step: what a zip archive is, from its entries.

use super::{Sniffed, ZipProbe};
use crate::kind::{
    ArchiveFormat, BookFormat, Family, FormatDetail, FormatKind, Mime, OfficeFormat, RasterFormat,
    from_extension,
};

/// How much of the `mimetype` entry is read: the longest media type is far shorter.
const MIMETYPE_LEN: usize = 128;

/// A zip's entry names and, when it has one, the start of its `mimetype` entry, supplied by the
/// caller that opened the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntries {
    names: Vec<String>,
    mimetype: Option<Vec<u8>>,
}

impl ZipEntries {
    /// The entries `names` (paths with `/` separators) and the first bytes of the `mimetype`
    /// entry; only its first 128 bytes are kept.
    pub fn new<I, S>(names: I, mimetype: Option<&[u8]>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        ZipEntries {
            names: names.into_iter().map(Into::into).collect(),
            mimetype: mimetype.map(|bytes| bytes[..bytes.len().min(MIMETYPE_LEN)].to_vec()),
        }
    }

    /// Whether an entry is called exactly `name`.
    fn has(&self, name: &str) -> bool {
        self.names.iter().any(|entry| entry == name)
    }

    /// Whether any entry is `directory` or lies inside it (its first path component).
    fn has_directory(&self, directory: &str) -> bool {
        self.names
            .iter()
            .any(|entry| entry.split('/').next() == Some(directory))
    }

    /// Whether any entry is a raster image, by its extension.
    fn has_image(&self) -> bool {
        self.names.iter().any(|entry| {
            let file = entry.rsplit('/').next().unwrap_or(entry);
            file.rsplit_once('.')
                .and_then(|(_, extension)| from_extension::<RasterFormat>(extension))
                .is_some()
        })
    }

    /// The `mimetype` entry's text, without the padding some writers add.
    fn mimetype(&self) -> Option<&str> {
        let bytes = self.mimetype.as_deref()?;
        std::str::from_utf8(bytes)
            .ok()
            .map(|text| text.trim_matches(|c: char| c.is_ascii_whitespace() || c == '\0'))
    }
}

/// What a zip is: an Office Open XML, OpenDocument or iWork document, an EPUB, a comic book, an
/// Android package or a Java archive, else a plain archive.
///
/// The `mimetype` entry decides first (EPUB, OpenDocument). `[Content_Types].xml` with a `word/`,
/// `xl/` or `ppt/` directory is Word, Excel or PowerPoint. An `Index` directory is an iWork
/// package, and its application comes from the file's extension, since the layout is the same.
/// `AndroidManifest.xml` with `classes.dex` is an APK; `META-INF/MANIFEST.MF` a JAR. A `.cbz` name
/// holding an image is a comic. Everything else is `Archive`.
pub fn sniff_zip(probe: ZipProbe, entries: &ZipEntries) -> Sniffed {
    let extension = probe.name().extension();
    let by_extension = extension.and_then(from_extension::<OfficeFormat>);

    if let Some(text) = entries.mimetype() {
        if let Some(format) = from_mime::<BookFormat>(text) {
            return book(format);
        }
        if let Some(format) = from_mime::<OfficeFormat>(text) {
            return office(format);
        }
    }
    if entries.has("[Content_Types].xml") {
        for (directory, format) in OPEN_XML {
            if entries.has_directory(directory) {
                return office(*format);
            }
        }
    }
    let apple = by_extension.filter(|format| is_iwork(*format));
    if let Some(format) = apple
        && (entries.has_directory("Index") || entries.has("Index.zip"))
    {
        return office(format);
    }
    if entries.has("AndroidManifest.xml") && entries.has("classes.dex") {
        return archive(ArchiveFormat::Apk);
    }
    if entries.has("META-INF/MANIFEST.MF") {
        return archive(ArchiveFormat::Jar);
    }
    let comic = extension.and_then(from_extension::<BookFormat>) == Some(BookFormat::Cbz);
    if comic && entries.has_image() {
        return book(BookFormat::Cbz);
    }
    if entries.has("mimetype") && entries.has("META-INF/container.xml") {
        return book(BookFormat::Epub);
    }
    archive(ArchiveFormat::Zip)
}

/// The directory each Office Open XML application keeps its parts in.
const OPEN_XML: &[(&str, OfficeFormat)] = &[
    ("word", OfficeFormat::Docx),
    ("xl", OfficeFormat::Xlsx),
    ("ppt", OfficeFormat::Pptx),
];

/// The format of family `F` whose media type is `text`.
fn from_mime<F: Family>(text: &str) -> Option<F> {
    F::ALL.iter().copied().find(|format| format.mime() == text)
}

fn is_iwork(format: OfficeFormat) -> bool {
    matches!(
        format,
        OfficeFormat::Pages | OfficeFormat::Numbers | OfficeFormat::Keynote
    )
}

fn office(format: OfficeFormat) -> Sniffed {
    Sniffed::new(
        format.kind(),
        Mime::known(format.mime()),
        FormatDetail::Office(format),
    )
}

fn book(format: BookFormat) -> Sniffed {
    Sniffed::new(
        FormatKind::Book,
        Mime::known(format.mime()),
        FormatDetail::Book(format),
    )
}

fn archive(format: ArchiveFormat) -> Sniffed {
    Sniffed::new(
        FormatKind::Archive,
        Mime::known(format.mime()),
        FormatDetail::Archive(format),
    )
}
