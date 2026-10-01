//! The first step: the file's own magic bytes.

use super::{FileHead, Sniffed, extension};
use crate::kind::{FormatDetail, FormatKind, Mime};
use infer::MatcherType;

/// What the magic bytes said.
#[derive(Debug)]
pub(super) enum Magic {
    /// A zip archive, whose entries say what it is.
    Zip,
    /// A type read from the bytes.
    Found(Sniffed),
}

/// The type the head's magic bytes name, or `None` when they name none (text, or a format with no
/// signature).
///
/// `infer` answers with the canonical extension of the type it matched; that extension is looked
/// up in the same families the name fallback uses, so a type has one table. A type `infer` knows
/// and no family holds (an executable, RAR) is `Other` with `infer`'s media type. Its text
/// matchers (HTML, XML, shell) are ignored: text is the next step's business.
pub(super) fn identify(head: &FileHead) -> Option<Magic> {
    let found = infer::get(head.bytes())?;
    if found.matcher_type() == MatcherType::Text {
        return None;
    }
    let canonical = found.extension();
    if canonical == "zip" {
        return Some(Magic::Zip);
    }
    let sniffed = extension::binary(canonical).unwrap_or_else(|| {
        let mime = Mime::parse(found.mime_type()).unwrap_or(Mime::OCTET_STREAM);
        Sniffed::new(FormatKind::Other, mime, FormatDetail::None)
    });
    Some(Magic::Found(sniffed))
}
