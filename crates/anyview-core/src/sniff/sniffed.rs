//! `Sniffed`: proof that a file's type was read from its bytes.

use crate::kind::{FormatDetail, FormatKind, Mime};
use crate::source::FileName;

/// A file whose type was established by `sniff`. Only this module's siblings can build one, so a
/// loader that takes a `Sniffed` is never handed a kind that was guessed from a name alone, and it
/// is not stored or sent: holding it means this process looked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniffed {
    kind: FormatKind,
    mime: Mime,
    detail: FormatDetail,
}

impl Sniffed {
    /// Only the sniffing code constructs one.
    pub(super) fn new(kind: FormatKind, mime: Mime, detail: FormatDetail) -> Self {
        Sniffed { kind, mime, detail }
    }

    /// What the file is.
    pub fn kind(&self) -> FormatKind {
        self.kind
    }

    /// Its media type.
    pub fn mime(&self) -> &Mime {
        &self.mime
    }

    /// The format inside the kind.
    pub fn detail(&self) -> &FormatDetail {
        &self.detail
    }
}

/// The answer of the first sniffing step: the type, or the news that the file is a zip whose
/// entries must be read to tell what kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SniffStep {
    /// The head was enough.
    Done(Sniffed),
    /// A zip: list its entries and call `sniff_zip`.
    LookInside(ZipProbe),
}

/// What the first step knew about a zip, handed back to `sniff_zip` with the entries. Only
/// `sniff` makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipProbe {
    name: FileName,
}

impl ZipProbe {
    pub(super) fn new(name: FileName) -> Self {
        ZipProbe { name }
    }

    /// The zip's own name, whose extension tells apart formats that share one layout.
    pub(super) fn name(&self) -> &FileName {
        &self.name
    }
}
