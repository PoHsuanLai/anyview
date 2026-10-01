//! Table tests for `sniff`, `sniff_zip` and `sniff_folder`, over small real signatures.

use super::*;
use crate::kind::{
    ArchiveFormat, BookFormat, Delimiter, FontFormat, FormatDetail, FormatKind, MediaContainer,
    OfficeFormat, RasterFormat, SyntaxName, TextEncoding, TreeFormat,
};
use crate::source::FileName;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\0\x10JFIF\0\x01\x01\0\0\x01\0\x01\0\0";
const GIF: &[u8] = b"GIF89a\x01\0\x01\0\x80\0\0";
const WEBP: &[u8] = b"RIFF\x1A\0\0\0WEBPVP8L\x0D\0\0\0/\0\0\0\0";
const BMP: &[u8] = b"BM\x1E\0\0\0\0\0\0\0\x1A\0\0\0\x0C\0\0\0\x01\0\x01\0\x01\0\x18\0";
const TIFF: &[u8] = b"II*\0\x08\0\0\0\0\0\0\0";
const ICO: &[u8] = b"\0\0\x01\0\x01\0\x10\x10\0\0\x01\0\x20\0";
const HEIC: &[u8] = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic";
const PSD: &[u8] = b"8BPS\0\x01\0\0\0\0\0\0";
const ICNS: &[u8] = b"icns\0\0\x10\0ic07\0\0\0\0";
const QOI: &[u8] = b"qoif\0\0\x02\0\0\0\x01\xE0\x03\0";
const TGA: &[u8] = b"\0\0\x02\0\0\0\0\0\0\0\0\0\x01\0\x01\0\x18\0";
const PDF: &[u8] = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";
const MP4: &[u8] = b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2mp41";
const MOV: &[u8] = b"\0\0\0\x14ftypqt  \0\0\0\0qt  \0\0\0\0";
const MKV: &[u8] = b"\x1A\x45\xDF\xA3\x93\x42\x82\x88matroska\x42\x87\x81\x04";
const WEBM: &[u8] = b"\x1A\x45\xDF\xA3\x9F\x42\x86\x81\x01\x42\xF7\x81\x01\x42\x82\x84webm";
const AVI: &[u8] = b"RIFF\x24\0\0\0AVI LIST";
const MPEG_TS: &[u8] = b"\x47\x40\x00\x10\0\0\xB0\x0D\0\x01\xC1\0\0";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";
const FLAC: &[u8] = b"fLaC\0\0\0\x22";
const WAV: &[u8] = b"RIFF\x24\0\0\0WAVEfmt ";
const AIFF: &[u8] = b"FORM\0\0\0\x26AIFFCOMM";
const OGG: &[u8] = b"OggS\0\x02\0\0\0\0\0\0\0\0\x12\x34\x56\x78\0\0\0\0\0\0\0\0\x01\x13vorbis";
const OPUS: &[u8] =
    b"OggS\0\x02\0\0\0\0\0\0\0\0\x12\x34\x56\x78\0\0\0\0\0\0\0\0\x01\x13OpusHead\x01\x02";
const M4A: &[u8] = b"\0\0\0\x20ftypM4A \0\0\0\0M4A mp42isom";
const TTF: &[u8] = b"\0\x01\0\0\0\x0F\x00\x80\0\x03\0\x30";
const OTF: &[u8] = b"OTTO\0\x0F\x01\0\0\x04\0\x20";
const WOFF2: &[u8] = b"wOF2\0\x01\0\0\0\0\x10\0";
const TTC: &[u8] = b"ttcf\0\x01\0\0\0\0\0\x02";
const GZIP: &[u8] = b"\x1F\x8B\x08\0\0\0\0\0\0\x03";
const SEVEN_ZIP: &[u8] = b"7z\xBC\xAF\x27\x1C\0\x04\x8D\x9B\xD5\x0F";
const XZ: &[u8] = b"\xFD7zXZ\0\0\x04\xE6\xD6\xB4F";
const ZSTD: &[u8] = b"\x28\xB5\x2F\xFD\x04\x58\0\0";
const BZIP2: &[u8] = b"BZh91AY&SY\0";
const RAR: &[u8] = b"Rar!\x1A\x07\x00\xCF\x90s\0\0\r\0\0\0\0\0\0\0";
/// An ELF header is 64 bytes; the signature library wants more than 52 of them.
const ELF: &[u8] = b"\x7FELF\x02\x01\x01\0\0\0\0\0\0\0\0\0\x03\0\x3E\0\x01\0\0\0\0\0\0\0\0\0\0\0\x40\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x40\0\x38\0\0\0\0\0\0\0\0\0";
const ZIP: &[u8] = b"PK\x03\x04\x14\0\0\0\0\0\0\0\0\0";
/// A ustar header: the signature sits at offset 257.
const TAR: &[u8] = &{
    let mut block = [0u8; 300];
    let signature = *b"ustar";
    let mut i = 0;
    while i < signature.len() {
        block[257 + i] = signature[i];
        i += 1;
    }
    block
};

struct Case {
    name: &'static str,
    file: &'static str,
    head: &'static [u8],
    kind: FormatKind,
    mime: &'static str,
    detail: FormatDetail,
}

const fn code(syntax: &'static str) -> FormatDetail {
    FormatDetail::Code(SyntaxName::known(syntax))
}

#[rustfmt::skip]
const CASES: &[Case] = &[
    // Magic bytes decide, whatever the name says.
    Case { name: "png", file: "a.png", head: PNG, kind: FormatKind::Raster, mime: "image/png", detail: FormatDetail::Raster(RasterFormat::Png) },
    Case { name: "png named txt", file: "a.txt", head: PNG, kind: FormatKind::Raster, mime: "image/png", detail: FormatDetail::Raster(RasterFormat::Png) },
    Case { name: "png with no name extension", file: "photo", head: PNG, kind: FormatKind::Raster, mime: "image/png", detail: FormatDetail::Raster(RasterFormat::Png) },
    Case { name: "jpeg", file: "a.jpg", head: JPEG, kind: FormatKind::Raster, mime: "image/jpeg", detail: FormatDetail::Raster(RasterFormat::Jpeg) },
    Case { name: "gif", file: "a.gif", head: GIF, kind: FormatKind::Raster, mime: "image/gif", detail: FormatDetail::Raster(RasterFormat::Gif) },
    Case { name: "webp", file: "a.webp", head: WEBP, kind: FormatKind::Raster, mime: "image/webp", detail: FormatDetail::Raster(RasterFormat::Webp) },
    Case { name: "bmp", file: "a.bmp", head: BMP, kind: FormatKind::Raster, mime: "image/bmp", detail: FormatDetail::Raster(RasterFormat::Bmp) },
    Case { name: "tiff", file: "a.tif", head: TIFF, kind: FormatKind::Raster, mime: "image/tiff", detail: FormatDetail::Raster(RasterFormat::Tiff) },
    Case { name: "ico", file: "a.ico", head: ICO, kind: FormatKind::Raster, mime: "image/vnd.microsoft.icon", detail: FormatDetail::Raster(RasterFormat::Ico) },
    Case { name: "heic", file: "a.heic", head: HEIC, kind: FormatKind::Raster, mime: "image/heic", detail: FormatDetail::Raster(RasterFormat::Heic) },
    Case { name: "psd", file: "a.psd", head: PSD, kind: FormatKind::Raster, mime: "image/vnd.adobe.photoshop", detail: FormatDetail::Raster(RasterFormat::Psd) },
    Case { name: "pdf", file: "a.pdf", head: PDF, kind: FormatKind::Pdf, mime: "application/pdf", detail: FormatDetail::None },
    Case { name: "pdf named png", file: "a.png", head: PDF, kind: FormatKind::Pdf, mime: "application/pdf", detail: FormatDetail::None },
    Case { name: "mp4", file: "a.mp4", head: MP4, kind: FormatKind::Video, mime: "video/mp4", detail: FormatDetail::Media(MediaContainer::Mp4) },
    Case { name: "mov", file: "a.mov", head: MOV, kind: FormatKind::Video, mime: "video/quicktime", detail: FormatDetail::Media(MediaContainer::Mov) },
    Case { name: "mkv", file: "a.mkv", head: MKV, kind: FormatKind::Video, mime: "video/x-matroska", detail: FormatDetail::Media(MediaContainer::Mkv) },
    Case { name: "webm", file: "a.webm", head: WEBM, kind: FormatKind::Video, mime: "video/webm", detail: FormatDetail::Media(MediaContainer::WebM) },
    Case { name: "avi", file: "a.avi", head: AVI, kind: FormatKind::Video, mime: "video/x-msvideo", detail: FormatDetail::Media(MediaContainer::Avi) },
    Case { name: "mp3", file: "a.mp3", head: MP3, kind: FormatKind::Audio, mime: "audio/mpeg", detail: FormatDetail::Media(MediaContainer::Mp3) },
    Case { name: "flac", file: "a.flac", head: FLAC, kind: FormatKind::Audio, mime: "audio/flac", detail: FormatDetail::Media(MediaContainer::Flac) },
    Case { name: "wav", file: "a.wav", head: WAV, kind: FormatKind::Audio, mime: "audio/wav", detail: FormatDetail::Media(MediaContainer::Wav) },
    Case { name: "aiff", file: "a.aiff", head: AIFF, kind: FormatKind::Audio, mime: "audio/aiff", detail: FormatDetail::Media(MediaContainer::Aiff) },
    Case { name: "ogg", file: "a.ogg", head: OGG, kind: FormatKind::Audio, mime: "audio/ogg", detail: FormatDetail::Media(MediaContainer::Ogg) },
    Case { name: "opus", file: "a.opus", head: OPUS, kind: FormatKind::Audio, mime: "audio/opus", detail: FormatDetail::Media(MediaContainer::Opus) },
    Case { name: "m4a", file: "a.m4a", head: M4A, kind: FormatKind::Audio, mime: "audio/mp4", detail: FormatDetail::Media(MediaContainer::M4a) },
    Case { name: "ttf", file: "a.ttf", head: TTF, kind: FormatKind::Font, mime: "font/ttf", detail: FormatDetail::Font(FontFormat::Ttf) },
    Case { name: "otf", file: "a.otf", head: OTF, kind: FormatKind::Font, mime: "font/otf", detail: FormatDetail::Font(FontFormat::Otf) },
    Case { name: "woff2", file: "a.woff2", head: WOFF2, kind: FormatKind::Font, mime: "font/woff2", detail: FormatDetail::Font(FontFormat::Woff2) },
    Case { name: "gzip", file: "a.tar.gz", head: GZIP, kind: FormatKind::Archive, mime: "application/gzip", detail: FormatDetail::Archive(ArchiveFormat::Gzip) },
    Case { name: "7z", file: "a.7z", head: SEVEN_ZIP, kind: FormatKind::Archive, mime: "application/x-7z-compressed", detail: FormatDetail::Archive(ArchiveFormat::SevenZip) },
    Case { name: "xz", file: "a.xz", head: XZ, kind: FormatKind::Archive, mime: "application/x-xz", detail: FormatDetail::Archive(ArchiveFormat::Xz) },
    Case { name: "zstd", file: "a.zst", head: ZSTD, kind: FormatKind::Archive, mime: "application/zstd", detail: FormatDetail::Archive(ArchiveFormat::Zstd) },
    Case { name: "bzip2", file: "a.bz2", head: BZIP2, kind: FormatKind::Archive, mime: "application/x-bzip2", detail: FormatDetail::Archive(ArchiveFormat::Bzip2) },
    Case { name: "tar", file: "a.tar", head: TAR, kind: FormatKind::Archive, mime: "application/x-tar", detail: FormatDetail::Archive(ArchiveFormat::Tar) },
    // A type no family holds keeps the signature library's media type.
    Case { name: "rar is not supported", file: "a.rar", head: RAR, kind: FormatKind::Other, mime: "application/vnd.rar", detail: FormatDetail::None },
    Case { name: "an executable", file: "run", head: ELF, kind: FormatKind::Other, mime: "application/x-executable", detail: FormatDetail::None },
    // Binary with no signature: the extension is the fallback.
    Case { name: "icns by extension", file: "a.icns", head: ICNS, kind: FormatKind::Raster, mime: "image/x-icns", detail: FormatDetail::Raster(RasterFormat::Icns) },
    Case { name: "qoi by extension", file: "a.qoi", head: QOI, kind: FormatKind::Raster, mime: "image/x-qoi", detail: FormatDetail::Raster(RasterFormat::Qoi) },
    Case { name: "tga by extension", file: "a.tga", head: TGA, kind: FormatKind::Raster, mime: "image/x-tga", detail: FormatDetail::Raster(RasterFormat::Tga) },
    Case { name: "ttc by extension", file: "a.ttc", head: TTC, kind: FormatKind::Font, mime: "font/collection", detail: FormatDetail::Font(FontFormat::Ttc) },
    Case { name: "transport stream by extension", file: "a.ts", head: MPEG_TS, kind: FormatKind::Video, mime: "video/mp2t", detail: FormatDetail::Media(MediaContainer::MpegTs) },
    Case { name: "binary with an unknown extension", file: "blob.bin", head: b"\0\x01\x02\x03", kind: FormatKind::Other, mime: "application/octet-stream", detail: FormatDetail::None },
    Case { name: "binary with no extension", file: "blob", head: b"\x01\x02\0\x03", kind: FormatKind::Other, mime: "application/octet-stream", detail: FormatDetail::None },
    Case { name: "binary named md stays binary", file: "a.md", head: b"\0\x01\x02\x03", kind: FormatKind::Other, mime: "application/octet-stream", detail: FormatDetail::None },
    // Text: the name picks among the text kinds.
    Case { name: "markdown", file: "README.md", head: b"# Title\n", kind: FormatKind::Markdown, mime: "text/markdown", detail: FormatDetail::None },
    Case { name: "markdown long extension", file: "notes.markdown", head: b"# Title\n", kind: FormatKind::Markdown, mime: "text/markdown", detail: FormatDetail::None },
    Case { name: "csv", file: "data.csv", head: b"a,b\n1,2\n", kind: FormatKind::Table, mime: "text/csv", detail: FormatDetail::Table(Delimiter::Comma) },
    Case { name: "tsv", file: "data.TSV", head: b"a\tb\n1\t2\n", kind: FormatKind::Table, mime: "text/tab-separated-values", detail: FormatDetail::Table(Delimiter::Tab) },
    Case { name: "json", file: "a.json", head: b"{\"a\":1}", kind: FormatKind::Tree, mime: "application/json", detail: FormatDetail::Tree(TreeFormat::Json) },
    Case { name: "json lines", file: "a.jsonl", head: b"{\"a\":1}\n{\"a\":2}\n", kind: FormatKind::Tree, mime: "application/x-ndjson", detail: FormatDetail::Tree(TreeFormat::JsonLines) },
    Case { name: "svg", file: "a.svg", head: b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>", kind: FormatKind::Vector, mime: "image/svg+xml", detail: FormatDetail::None },
    Case { name: "html is code", file: "a.html", head: b"<!DOCTYPE html><html></html>", kind: FormatKind::Code, mime: "text/html", detail: code("html") },
    Case { name: "xml is code", file: "a.xml", head: b"<?xml version=\"1.0\"?><a/>", kind: FormatKind::Code, mime: "application/xml", detail: code("xml") },
    Case { name: "shell script", file: "run.sh", head: b"#!/bin/sh\necho hi\n", kind: FormatKind::Code, mime: "text/plain", detail: code("shell") },
    Case { name: "rust", file: "main.rs", head: b"fn main() {}\n", kind: FormatKind::Code, mime: "text/plain", detail: code("rust") },
    Case { name: "rust upper case extension", file: "lib.RS", head: b"fn f() {}\n", kind: FormatKind::Code, mime: "text/plain", detail: code("rust") },
    Case { name: "typescript, not a transport stream", file: "a.ts", head: b"const a: number = 1;\n", kind: FormatKind::Code, mime: "text/plain", detail: code("typescript") },
    Case { name: "makefile by name", file: "Makefile", head: b"all:\n\ttrue\n", kind: FormatKind::Code, mime: "text/plain", detail: code("makefile") },
    Case { name: "dockerfile by name", file: "dockerfile", head: b"FROM scratch\n", kind: FormatKind::Code, mime: "text/plain", detail: code("dockerfile") },
    Case { name: "plain text", file: "notes.txt", head: b"hello\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "text with no extension", file: "README", head: b"hello\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "text with an unknown extension", file: "a.xyz", head: b"hello\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "utf-16 text", file: "a.txt", head: b"\xFF\xFEh\0i\0", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf16Le) },
    Case { name: "an empty file", file: "empty.txt", head: b"", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "an empty file named png is not an image", file: "empty.png", head: b"", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "text named png is not an image", file: "a.png", head: b"not really a png\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    // Extensions are whole tokens: a suffix or a middle part is not one.
    Case { name: "extension is the last token", file: "photo.png.txt", head: b"hello\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "a name ending in rs has no extension", file: "myrs", head: b"hello\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "a longer extension is not rs", file: "a.rsx", head: b"hello\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "mdx is not md", file: "a.mdx", head: b"# hi\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "a backup of a makefile is not a makefile", file: "Makefile.bak", head: b"all:\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
    Case { name: "a name ending in makefile is not one", file: "notmakefile", head: b"all:\n", kind: FormatKind::PlainText, mime: "text/plain", detail: FormatDetail::Text(TextEncoding::Utf8) },
];

fn name(file: &str) -> FileName {
    FileName::new(file).unwrap()
}

#[test]
fn sniff_names_the_kind_mime_and_detail() {
    for case in CASES {
        let step = sniff(&FileHead::new(case.head), &name(case.file));
        let SniffStep::Done(sniffed) = step else {
            panic!("{}: expected an answer, got {step:?}", case.name);
        };
        assert_eq!(sniffed.kind(), case.kind, "{} kind", case.name);
        assert_eq!(sniffed.mime().as_str(), case.mime, "{} mime", case.name);
        assert_eq!(sniffed.detail(), &case.detail, "{} detail", case.name);
    }
}

#[test]
fn a_zip_signature_asks_for_the_entries_whatever_the_name() {
    const FILES: &[&str] = &[
        "a.zip",
        "a.docx",
        "a.epub",
        "a.jar",
        "a.pages",
        "noextension",
    ];
    for file in FILES {
        let step = sniff(&FileHead::new(ZIP), &name(file));
        assert!(matches!(step, SniffStep::LookInside(_)), "{file}: {step:?}");
    }
}

#[test]
fn a_head_is_cut_to_four_kibibytes_before_sniffing() {
    let mut bytes = vec![b'a'; 5000];
    bytes[4500] = 0; // a NUL past the head must not matter
    let step = sniff(&FileHead::new(&bytes), &name("a.txt"));
    let SniffStep::Done(sniffed) = step else {
        panic!("{step:?}")
    };
    assert_eq!(sniffed.kind(), FormatKind::PlainText);
    bytes[10] = 0; // a NUL inside it makes the file binary
    let step = sniff(&FileHead::new(&bytes), &name("a.txt"));
    let SniffStep::Done(sniffed) = step else {
        panic!("{step:?}")
    };
    assert_eq!(sniffed.kind(), FormatKind::Other);
}

#[test]
fn a_folder_is_sniffed_without_bytes() {
    let folder = sniff_folder();
    assert_eq!(folder.kind(), FormatKind::Folder);
    assert_eq!(folder.mime().as_str(), "inode/directory");
    assert_eq!(folder.detail(), &FormatDetail::None);
}

struct ZipCase {
    name: &'static str,
    file: &'static str,
    entries: &'static [&'static str],
    mimetype: Option<&'static [u8]>,
    kind: FormatKind,
    detail: FormatDetail,
}

const IWORK: &[&str] = &[
    "Index/Document.iwa",
    "Metadata/Properties.plist",
    "preview.jpg",
];
const APK: &[&str] = &["AndroidManifest.xml", "classes.dex", "META-INF/MANIFEST.MF"];
const OPEN_DOCUMENT: &[&str] = &["mimetype", "content.xml", "META-INF/manifest.xml"];

const fn office(format: OfficeFormat) -> FormatDetail {
    FormatDetail::Office(format)
}

const fn archive(format: ArchiveFormat) -> FormatDetail {
    FormatDetail::Archive(format)
}

#[rustfmt::skip]
const ZIP_CASES: &[ZipCase] = &[
    ZipCase { name: "epub by its mimetype", file: "b.epub", entries: &["mimetype", "META-INF/container.xml", "OEBPS/content.opf"], mimetype: Some(b"application/epub+zip"), kind: FormatKind::Book, detail: FormatDetail::Book(BookFormat::Epub) },
    ZipCase { name: "epub whatever its name", file: "b.zip", entries: &["mimetype"], mimetype: Some(b"application/epub+zip"), kind: FormatKind::Book, detail: FormatDetail::Book(BookFormat::Epub) },
    ZipCase { name: "epub without the mimetype bytes", file: "b.epub", entries: &["mimetype", "META-INF/container.xml"], mimetype: None, kind: FormatKind::Book, detail: FormatDetail::Book(BookFormat::Epub) },
    ZipCase { name: "odt", file: "a.odt", entries: OPEN_DOCUMENT, mimetype: Some(b"application/vnd.oasis.opendocument.text"), kind: FormatKind::Office, detail: office(OfficeFormat::Odt) },
    ZipCase { name: "ods", file: "a.ods", entries: OPEN_DOCUMENT, mimetype: Some(b"application/vnd.oasis.opendocument.spreadsheet"), kind: FormatKind::Office, detail: office(OfficeFormat::Ods) },
    ZipCase { name: "odp", file: "a.odp", entries: OPEN_DOCUMENT, mimetype: Some(b"application/vnd.oasis.opendocument.presentation"), kind: FormatKind::Office, detail: office(OfficeFormat::Odp) },
    ZipCase { name: "odt with a newline after the mimetype", file: "a.odt", entries: OPEN_DOCUMENT, mimetype: Some(b"application/vnd.oasis.opendocument.text\n"), kind: FormatKind::Office, detail: office(OfficeFormat::Odt) },
    ZipCase { name: "an unknown mimetype falls through", file: "a.zip", entries: &["mimetype", "a.txt"], mimetype: Some(b"application/x-unknown"), kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "docx", file: "a.docx", entries: &["[Content_Types].xml", "word/document.xml"], mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Docx) },
    ZipCase { name: "docx renamed zip", file: "a.zip", entries: &["[Content_Types].xml", "word/document.xml"], mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Docx) },
    ZipCase { name: "xlsx", file: "a.xlsx", entries: &["[Content_Types].xml", "xl/workbook.xml"], mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Xlsx) },
    ZipCase { name: "pptx", file: "a.pptx", entries: &["[Content_Types].xml", "ppt/presentation.xml"], mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Pptx) },
    ZipCase { name: "open packaging with no office part", file: "a.xps", entries: &["[Content_Types].xml", "Documents/1/Pages/1.fpage"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "a directory that only starts with word", file: "a.docx", entries: &["[Content_Types].xml", "wordy/document.xml"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "word below another directory", file: "a.docx", entries: &["[Content_Types].xml", "other/word/document.xml"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "pages", file: "a.pages", entries: IWORK, mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Pages) },
    ZipCase { name: "numbers", file: "a.numbers", entries: IWORK, mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Numbers) },
    ZipCase { name: "keynote", file: "a.key", entries: IWORK, mimetype: None, kind: FormatKind::Office, detail: office(OfficeFormat::Keynote) },
    ZipCase { name: "an iwork layout under a zip name", file: "a.zip", entries: IWORK, mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "a pages name without the index", file: "a.pages", entries: &["a.txt"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "a directory that only starts with index", file: "a.pages", entries: &["Indexer/a.iwa"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "apk", file: "a.apk", entries: APK, mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Apk) },
    ZipCase { name: "apk renamed zip", file: "a.zip", entries: APK, mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Apk) },
    ZipCase { name: "jar", file: "a.jar", entries: &["META-INF/MANIFEST.MF", "com/x/A.class"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Jar) },
    ZipCase { name: "cbz", file: "c.cbz", entries: &["001.jpg", "002.png"], mimetype: None, kind: FormatKind::Book, detail: FormatDetail::Book(BookFormat::Cbz) },
    ZipCase { name: "cbz with an upper case image in a folder", file: "c.CBZ", entries: &["pages/Page 1.PNG"], mimetype: None, kind: FormatKind::Book, detail: FormatDetail::Book(BookFormat::Cbz) },
    ZipCase { name: "cbz name with no image", file: "c.cbz", entries: &["notes.txt"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "images in a zip name are not a comic", file: "c.zip", entries: &["001.jpg"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "an entry that merely ends in an image extension", file: "c.cbz", entries: &["notjpg", "a.jpgx"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "a plain zip", file: "a.zip", entries: &["a.txt", "b/c.txt"], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
    ZipCase { name: "an empty zip", file: "a.zip", entries: &[], mimetype: None, kind: FormatKind::Archive, detail: archive(ArchiveFormat::Zip) },
];

fn probe(file: &str) -> ZipProbe {
    match sniff(&FileHead::new(ZIP), &name(file)) {
        SniffStep::LookInside(probe) => probe,
        SniffStep::Done(sniffed) => panic!("{file}: a zip head was answered as {sniffed:?}"),
    }
}

#[test]
fn sniff_zip_recognises_the_formats_inside_a_zip() {
    for case in ZIP_CASES {
        let entries = ZipEntries::new(case.entries.iter().copied(), case.mimetype);
        let sniffed = sniff_zip(probe(case.file), &entries);
        assert_eq!(sniffed.kind(), case.kind, "{} kind", case.name);
        assert_eq!(sniffed.detail(), &case.detail, "{} detail", case.name);
    }
}

#[test]
fn sniff_zip_states_the_media_type_of_what_it_found() {
    // name, file, entries, mimetype entry, media type
    type Row = (
        &'static str,
        &'static str,
        &'static [&'static str],
        Option<&'static [u8]>,
        &'static str,
    );
    #[rustfmt::skip]
    const CASES: &[Row] = &[
        (
            "docx",
            "a.docx",
            &["[Content_Types].xml", "word/a.xml"],
            None,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        (
            "epub",
            "a.epub",
            &["mimetype"],
            Some(b"application/epub+zip"),
            "application/epub+zip",
        ),
        ("plain", "a.zip", &["a.txt"], None, "application/zip"),
    ];
    for (name, file, entries, mimetype, want) in CASES {
        let entries = ZipEntries::new(entries.iter().copied(), *mimetype);
        assert_eq!(
            sniff_zip(probe(file), &entries).mime().as_str(),
            *want,
            "{name}"
        );
    }
}

#[test]
fn only_the_first_bytes_of_the_mimetype_entry_are_read() {
    let mut long = b"application/epub+zip".to_vec();
    long.extend(std::iter::repeat_n(b' ', 200));
    long.extend_from_slice(b"junk");
    let entries = ZipEntries::new(["mimetype"], Some(&long));
    assert_eq!(sniff_zip(probe("b.zip"), &entries).kind(), FormatKind::Book);
}

#[test]
fn the_detail_tables_hold_every_fixture_kind() {
    // Every case's detail agrees with its kind, so a table row cannot name a kind that its
    // detail contradicts.
    for case in CASES {
        let agrees = match (&case.detail, case.kind) {
            (FormatDetail::Raster(_), FormatKind::Raster)
            | (FormatDetail::Code(_), FormatKind::Code)
            | (FormatDetail::Table(_), FormatKind::Table)
            | (FormatDetail::Tree(_), FormatKind::Tree)
            | (FormatDetail::Text(_), FormatKind::PlainText)
            | (FormatDetail::Media(_), FormatKind::Video | FormatKind::Audio)
            | (FormatDetail::Font(_), FormatKind::Font)
            | (FormatDetail::Archive(_), FormatKind::Archive) => true,
            (FormatDetail::None, kind) => matches!(
                kind,
                FormatKind::Pdf
                    | FormatKind::Vector
                    | FormatKind::Markdown
                    | FormatKind::Folder
                    | FormatKind::Other
            ),
            (FormatDetail::Book(_) | FormatDetail::Office(_), _)
            | (FormatDetail::Raster(_), _)
            | (FormatDetail::Code(_), _)
            | (FormatDetail::Table(_), _)
            | (FormatDetail::Tree(_), _)
            | (FormatDetail::Text(_), _)
            | (FormatDetail::Media(_), _)
            | (FormatDetail::Font(_), _)
            | (FormatDetail::Archive(_), _) => false,
        };
        assert!(
            agrees,
            "{}: {:?} with {:?}",
            case.name, case.kind, case.detail
        );
    }
}
