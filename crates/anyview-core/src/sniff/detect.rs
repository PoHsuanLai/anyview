//! The two sniffing entry points.

use super::magic::{self, Magic};
use super::{FileHead, SniffStep, Sniffed, ZipProbe, extension, text};
use crate::kind::{Family, FormatDetail, FormatKind, Mime, RasterFormat, from_extension};
use crate::source::FileName;

/// What a file is, from its first bytes and its name.
///
/// Magic bytes decide first. A head with no signature is text when it holds no NUL and is valid
/// UTF-8, or starts with a UTF-16 byte-order mark; the name then picks Markdown, a table, a tree,
/// SVG, a syntax or plain text. Any other head is binary, and the name picks among the binary
/// families or leaves it `Other`. A zip is not decided here: the answer asks for its entries.
pub fn sniff(head: &FileHead, name: &FileName) -> SniffStep {
    match magic::identify(head) {
        Some(Magic::Zip) => SniffStep::LookInside(ZipProbe::new(name.clone())),
        Some(Magic::Found(sniffed)) => SniffStep::Done(raw_over_container(sniffed, name)),
        None => SniffStep::Done(by_name(head, name)),
    }
}

/// A camera raw file is a TIFF (most makers) or an ISO base media file (Canon's CR3) underneath, so
/// its magic bytes name the container. A name that is a raw extension says which it is.
fn raw_over_container(sniffed: Sniffed, name: &FileName) -> Sniffed {
    let container = matches!(
        (sniffed.kind(), sniffed.detail()),
        (FormatKind::Raster, FormatDetail::Raster(RasterFormat::Tiff))
            | (FormatKind::Video, FormatDetail::Media(_))
    );
    let raw = name
        .extension()
        .and_then(from_extension::<RasterFormat>)
        .filter(|format| *format == RasterFormat::Raw);
    match (container, raw) {
        (true, Some(format)) => Sniffed::new(
            FormatKind::Raster,
            Mime::known(format.mime()),
            FormatDetail::Raster(format),
        ),
        (false, _) | (true, None) => sniffed,
    }
}

/// A folder, which has no bytes to read.
pub fn sniff_folder() -> Sniffed {
    Sniffed::new(
        FormatKind::Folder,
        Mime::known("inode/directory"),
        FormatDetail::None,
    )
}

/// The type of a head with no signature: text by its name, binary by its extension.
fn by_name(head: &FileHead, name: &FileName) -> Sniffed {
    match text::encoding(head) {
        Some(encoding) => extension::text(name, encoding),
        None => name
            .extension()
            .and_then(extension::binary)
            .unwrap_or_else(extension::unknown),
    }
}
