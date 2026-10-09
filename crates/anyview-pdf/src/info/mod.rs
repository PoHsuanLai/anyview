//! What a PDF says about itself: its Info dictionary (or its XMP packet), version, page size,
//! protection and the rest the Info panel lists. Read once, on the worker that opened the file.

mod date;
mod facts;
mod page_format;
mod xmp;

pub use page_format::{Orientation, PageFormat};

use crate::document::PdfDocument;
use anyview_core::{FactTime, PageCount};

/// A PDF's version as its header declares it: `1.7` is major 1, minor 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormatVersion {
    /// The digit before the point.
    pub major: u8,
    /// The digit after it.
    pub minor: u8,
}

/// What a document's owner has switched off for a reader who opened it without the owner
/// password.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Restriction {
    /// Printing.
    Printing,
    /// Copying text and pictures out.
    Copying,
    /// Changing the contents.
    Editing,
    /// Adding comments and filling forms.
    Commenting,
}

/// Whether the file is encrypted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Protection {
    /// Not encrypted.
    Open,
    /// Encrypted with a password a reader does not need to open it, so it opened; the rights the
    /// owner took away from such a reader, in a fixed order, none when it kept them all.
    Encrypted(Vec<Restriction>),
}

/// Whether the document carries a structure tree for assistive technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tagging {
    /// It does.
    Tagged,
    /// It does not.
    Untagged,
}

/// The facts of an open PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfInfo {
    /// The title, from the Info dictionary or else the XMP packet.
    pub title: Option<String>,
    /// The author, from the same.
    pub author: Option<String>,
    /// What it is about.
    pub subject: Option<String>,
    /// The words it is tagged with.
    pub keywords: Option<String>,
    /// The program the content came from.
    pub creator: Option<String>,
    /// The program that wrote the PDF.
    pub producer: Option<String>,
    /// When the document was made, from its Info dictionary.
    pub created: Option<FactTime>,
    /// When it was last changed, from the same.
    pub modified: Option<FactTime>,
    /// The version in the header.
    pub version: Option<FormatVersion>,
    /// How many pages it has.
    pub pages: PageCount,
    /// How big its pages are.
    pub page_size: PageFormat,
    /// Whether and how it is protected.
    pub protection: Protection,
    /// Whether it is tagged.
    pub tagging: Tagging,
    /// How many files are embedded in it.
    pub attachments: usize,
    /// How many digital signatures it carries.
    pub signatures: usize,
}

impl PdfDocument {
    /// What the document says about itself. Blocking and cheap: one pass over the Info
    /// dictionary, the page sizes the document already holds, and the first page's tag check.
    pub fn info(&self) -> PdfInfo {
        let doc = self.inner();
        let meta = doc.metadata();
        let xmp = doc
            .xmp_metadata()
            .map(|packet| xmp::read(&packet))
            .unwrap_or_default();
        let permissions = doc.permissions();
        let protection = if doc.is_encrypted() {
            Protection::Encrypted(
                [
                    (permissions.print, Restriction::Printing),
                    (permissions.copy, Restriction::Copying),
                    (permissions.modify, Restriction::Editing),
                    (permissions.annotate, Restriction::Commenting),
                ]
                .into_iter()
                .filter_map(|(allowed, restriction)| (!allowed).then_some(restriction))
                .collect(),
            )
        } else {
            Protection::Open
        };
        let tagged = doc
            .page(0)
            .ok()
            .is_some_and(|first| first.structure().is_some());
        PdfInfo {
            title: meta.title.or(xmp.title),
            author: meta.author.or(xmp.author),
            subject: meta.subject.or(xmp.subject),
            keywords: meta.keywords.or(xmp.keywords),
            creator: meta.creator,
            producer: meta.producer,
            created: meta.creation_date.as_deref().and_then(date::parse),
            modified: meta.modification_date.as_deref().and_then(date::parse),
            version: doc.version().map(|v| FormatVersion {
                major: v.major,
                minor: v.minor,
            }),
            pages: self.page_count(),
            page_size: page_format::of_pages(self.page_sizes()),
            protection,
            tagging: if tagged {
                Tagging::Tagged
            } else {
                Tagging::Untagged
            },
            attachments: doc.attachments().len(),
            signatures: doc.signatures().len(),
        }
    }
}
