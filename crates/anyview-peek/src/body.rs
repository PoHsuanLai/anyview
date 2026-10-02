//! What a peek produced, with its type erased: the one value the pane draws.

use crate::described::Described;
use crate::error::PeekError;
use crate::folder::FolderSummary;
use crate::pdf::PdfPeeked;
use anyview_archive::ArchivePeeked;
use anyview_core::Peek;
use anyview_font::FontPeeked;
use anyview_image::ImagePeek;
use anyview_text::{CodePeeked, MarkdownPeeked, PlainPeeked, TablePeeked, TreePeeked};
use std::sync::Arc;

/// The result of one peek, whichever kind made it. A peek's own `Peeked` converts into the variant
/// that holds it (`From`), so the registry needs no second match over kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A downscaled picture: a raster image or an SVG drawn at the budget's size. Shared, so the
    /// pane can tell the picture it has uploaded from a new one without comparing pixels.
    Picture(Arc<ImagePeek>),
    /// A PDF's first page.
    Page(PdfPeeked),
    /// The first lines of plain text.
    Plain(PlainPeeked),
    /// The first lines of code, highlighted.
    Code(CodePeeked),
    /// The start of a Markdown document, rendered.
    Markdown(MarkdownPeeked),
    /// The first rows of a table.
    Table(TablePeeked),
    /// The top level of a JSON tree.
    Tree(TreePeeked),
    /// An archive's first entries.
    Archive(ArchivePeeked),
    /// A font's specimen.
    Font(FontPeeked),
    /// A folder's counts.
    Folder(FolderSummary),
    /// Nothing to draw but the facts: the kind has no back end yet.
    FactsOnly(Described),
    /// The peek failed; this is why, in words, and the pane shows the facts beside it.
    Unavailable(String),
}

impl From<ImagePeek> for Body {
    fn from(peeked: ImagePeek) -> Self {
        Body::Picture(Arc::new(peeked))
    }
}

impl From<PdfPeeked> for Body {
    fn from(peeked: PdfPeeked) -> Self {
        Body::Page(peeked)
    }
}

impl From<PlainPeeked> for Body {
    fn from(peeked: PlainPeeked) -> Self {
        Body::Plain(peeked)
    }
}

impl From<CodePeeked> for Body {
    fn from(peeked: CodePeeked) -> Self {
        Body::Code(peeked)
    }
}

impl From<MarkdownPeeked> for Body {
    fn from(peeked: MarkdownPeeked) -> Self {
        Body::Markdown(peeked)
    }
}

impl From<TablePeeked> for Body {
    fn from(peeked: TablePeeked) -> Self {
        Body::Table(peeked)
    }
}

impl From<TreePeeked> for Body {
    fn from(peeked: TreePeeked) -> Self {
        Body::Tree(peeked)
    }
}

impl From<ArchivePeeked> for Body {
    fn from(peeked: ArchivePeeked) -> Self {
        Body::Archive(peeked)
    }
}

impl From<FontPeeked> for Body {
    fn from(peeked: FontPeeked) -> Self {
        Body::Font(peeked)
    }
}

impl From<FolderSummary> for Body {
    fn from(peeked: FolderSummary) -> Self {
        Body::Folder(peeked)
    }
}

impl From<Described> for Body {
    fn from(peeked: Described) -> Self {
        Body::FactsOnly(peeked)
    }
}

/// A [`Peek`] the registry can run: its result becomes a [`Body`] and its error a [`PeekError`].
/// Every implementation in this crate and in the back ends qualifies; the bounds are all it is.
pub trait Light: Peek<Peeked: Into<Body>, Error: Into<PeekError>> {}

impl<P> Light for P where P: Peek<Peeked: Into<Body>, Error: Into<PeekError>> {}

impl Body {
    /// The word for this body: what the pane stamps as `data-body`.
    pub fn slug(&self) -> &'static str {
        match self {
            Body::Picture(_) => "picture",
            Body::Page(_) => "page",
            Body::Plain(_) => "plain",
            Body::Code(_) => "code",
            Body::Markdown(_) => "markdown",
            Body::Table(_) => "table",
            Body::Tree(_) => "tree",
            Body::Archive(_) => "archive",
            Body::Font(_) => "font",
            Body::Folder(_) => "folder",
            Body::FactsOnly(_) => "facts",
            Body::Unavailable(_) => "unavailable",
        }
    }
}
