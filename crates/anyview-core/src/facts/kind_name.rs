//! The name a person gives a kind of file: `JPEG image`, `Rust source`. Finder's "Kind", not the
//! media type.

use crate::kind::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, FormatDetail, FormatKind, MediaContainer,
    OfficeFormat, RasterFormat, TreeFormat,
};
use crate::sniff::Sniffed;

/// `JPEG image`, `PDF document`, `Rust source`: what `sniffed` is, in a person's words.
pub fn kind_name(sniffed: &Sniffed) -> String {
    match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Pdf, _) => "PDF document".to_owned(),
        (FormatKind::Raster, FormatDetail::Raster(format)) => raster(*format).to_owned(),
        (FormatKind::Raster, _) => "Image".to_owned(),
        (FormatKind::Vector, _) => "SVG image".to_owned(),
        (FormatKind::Video, FormatDetail::Media(container)) => media(*container, "video"),
        (FormatKind::Audio, FormatDetail::Media(container)) => media(*container, "audio"),
        (FormatKind::Video, _) => "Video".to_owned(),
        (FormatKind::Audio, _) => "Audio".to_owned(),
        (FormatKind::Markdown, _) => "Markdown document".to_owned(),
        (FormatKind::Code, FormatDetail::Code(syntax)) => {
            format!("{} source", language(syntax.as_str()))
        }
        (FormatKind::Code, _) => "Source code".to_owned(),
        (FormatKind::PlainText, _) => "Plain text".to_owned(),
        (FormatKind::Table, FormatDetail::Table(Delimiter::Comma)) => "CSV table".to_owned(),
        (FormatKind::Table, FormatDetail::Table(Delimiter::Tab)) => "TSV table".to_owned(),
        (FormatKind::Table, FormatDetail::Office(format)) => office(*format).to_owned(),
        (FormatKind::Table, _) => "Table".to_owned(),
        (FormatKind::Tree, FormatDetail::Tree(TreeFormat::Json)) => "JSON document".to_owned(),
        (FormatKind::Tree, FormatDetail::Tree(TreeFormat::JsonLines)) => {
            "JSON Lines document".to_owned()
        }
        (FormatKind::Tree, _) => "Structured data".to_owned(),
        (FormatKind::Font, FormatDetail::Font(format)) => font(*format).to_owned(),
        (FormatKind::Font, _) => "Font".to_owned(),
        (FormatKind::Archive, FormatDetail::Archive(format)) => archive(*format).to_owned(),
        (FormatKind::Archive, _) => "Archive".to_owned(),
        (FormatKind::Book, FormatDetail::Book(BookFormat::Epub)) => "EPUB book".to_owned(),
        (FormatKind::Book, FormatDetail::Book(BookFormat::Cbz)) => "Comic book archive".to_owned(),
        (FormatKind::Book, _) => "Book".to_owned(),
        (FormatKind::Office, FormatDetail::Office(format)) => office(*format).to_owned(),
        (FormatKind::Office, _) => "Document".to_owned(),
        (FormatKind::Folder, _) => "Folder".to_owned(),
        (FormatKind::Other, _) => "File".to_owned(),
    }
}

fn raster(format: RasterFormat) -> &'static str {
    match format {
        RasterFormat::Png => "PNG image",
        RasterFormat::Jpeg => "JPEG image",
        RasterFormat::Gif => "GIF image",
        RasterFormat::Webp => "WebP image",
        RasterFormat::Bmp => "BMP image",
        RasterFormat::Tiff => "TIFF image",
        RasterFormat::Ico => "Windows icon",
        RasterFormat::Tga => "TGA image",
        RasterFormat::Qoi => "QOI image",
        RasterFormat::Avif => "AVIF image",
        RasterFormat::Jxl => "JPEG XL image",
        RasterFormat::Heic => "HEIC image",
        RasterFormat::Psd => "Photoshop document",
        RasterFormat::Icns => "Apple icon",
        RasterFormat::Exr => "OpenEXR image",
        RasterFormat::Hdr => "Radiance HDR image",
        RasterFormat::Raw => "RAW image",
    }
}

fn media(container: MediaContainer, noun: &str) -> String {
    let name = match container {
        MediaContainer::Mkv => "Matroska".to_owned(),
        MediaContainer::WebM => "WebM".to_owned(),
        MediaContainer::Mov => "QuickTime".to_owned(),
        MediaContainer::MpegTs => "MPEG-TS".to_owned(),
        MediaContainer::Mp4
        | MediaContainer::M4v
        | MediaContainer::Avi
        | MediaContainer::Ogv
        | MediaContainer::Mpeg
        | MediaContainer::Wmv
        | MediaContainer::Flv
        | MediaContainer::Mp3
        | MediaContainer::Aac
        | MediaContainer::M4a
        | MediaContainer::Flac
        | MediaContainer::Wav
        | MediaContainer::Aiff
        | MediaContainer::Ogg
        | MediaContainer::Opus => container.extension().to_ascii_uppercase(),
    };
    format!("{name} {noun}")
}

fn language(syntax: &str) -> String {
    match syntax {
        "c" => "C".to_owned(),
        "c++" => "C++".to_owned(),
        "c#" => "C#".to_owned(),
        "css" => "CSS".to_owned(),
        "scss" => "SCSS".to_owned(),
        "html" => "HTML".to_owned(),
        "xml" => "XML".to_owned(),
        "sql" => "SQL".to_owned(),
        "php" => "PHP".to_owned(),
        "toml" => "TOML".to_owned(),
        "yaml" => "YAML".to_owned(),
        "ini" => "INI".to_owned(),
        "javascript" => "JavaScript".to_owned(),
        "typescript" => "TypeScript".to_owned(),
        "shell" => "Shell".to_owned(),
        other => {
            let mut chars = other.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    }
}

fn office(format: OfficeFormat) -> &'static str {
    match format {
        OfficeFormat::Docx => "Word document",
        OfficeFormat::Xlsx => "Excel spreadsheet",
        OfficeFormat::Pptx => "PowerPoint presentation",
        OfficeFormat::Odt => "OpenDocument text",
        OfficeFormat::Ods => "OpenDocument spreadsheet",
        OfficeFormat::Odp => "OpenDocument presentation",
        OfficeFormat::Odg => "OpenDocument drawing",
        OfficeFormat::Pages => "Pages document",
        OfficeFormat::Numbers => "Numbers spreadsheet",
        OfficeFormat::Keynote => "Keynote presentation",
        OfficeFormat::Doc => "Word 97-2003 document",
        OfficeFormat::Xls => "Excel 97-2003 spreadsheet",
        OfficeFormat::Ppt => "PowerPoint 97-2003 presentation",
    }
}

fn font(format: FontFormat) -> &'static str {
    match format {
        FontFormat::Ttf => "TrueType font",
        FontFormat::Otf => "OpenType font",
        FontFormat::Ttc => "Font collection",
        FontFormat::Woff => "WOFF font",
        FontFormat::Woff2 => "WOFF2 font",
    }
}

fn archive(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "ZIP archive",
        ArchiveFormat::Tar => "Tar archive",
        ArchiveFormat::Gzip => "Gzip compressed file",
        ArchiveFormat::Bzip2 => "Bzip2 compressed file",
        ArchiveFormat::Xz => "XZ compressed file",
        ArchiveFormat::Zstd => "Zstandard compressed file",
        ArchiveFormat::SevenZip => "7-Zip archive",
        ArchiveFormat::Jar => "Java archive",
        ArchiveFormat::Apk => "Android package",
    }
}
