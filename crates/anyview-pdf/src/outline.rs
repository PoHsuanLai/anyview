//! The document's outline (its bookmarks) and the links on a page.

use crate::document::PdfDocument;
use crate::error::PdfError;
use crate::geometry::{PageRect, displayed};
use anyview_core::PageIndex;
use pdfrum::LinkTarget as Target;

/// Whether an outline entry has children and shows them when the outline first opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Disclosure {
    /// It has no children.
    Leaf,
    /// Its children show.
    Open,
    /// Its children are folded away.
    Closed,
}

/// One bookmark. The outline is a list in reading order; `depth` gives its tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    /// The text shown.
    pub title: String,
    /// How many levels down it sits; 0 is the top.
    pub depth: u32,
    /// The page it goes to, when it goes to a page of this document.
    pub page: Option<PageIndex>,
    /// Whether its children show at first.
    pub disclosure: Disclosure,
}

/// The document's bookmarks in reading order; empty when it has none.
pub fn outline(doc: &PdfDocument) -> Vec<OutlineEntry> {
    let count = doc.page_count().get();
    let marks: Vec<_> = doc.inner().outline().iter().collect();
    marks
        .iter()
        .enumerate()
        .map(|(at, mark)| {
            let has_children = marks
                .get(at + 1)
                .is_some_and(|next| next.depth() > mark.depth());
            OutlineEntry {
                title: mark.title(),
                depth: u32::try_from(mark.depth()).unwrap_or(u32::MAX),
                page: mark
                    .page_index()
                    .map(|page| PageIndex(u32::from(page)))
                    .filter(|page| page.0 < count),
                disclosure: match (has_children, mark.is_open()) {
                    (false, _) => Disclosure::Leaf,
                    (true, true) => Disclosure::Open,
                    (true, false) => Disclosure::Closed,
                },
            }
        })
        .collect()
}

/// Where a link on a page leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// Another page of this document.
    Page(PageIndex),
    /// A web or mail address. Nothing here opens it: the viewer decides whether to.
    Uri(String),
    /// A named action, a script, another file, or a place that is not a page of this document.
    Other,
}

/// A link on a page: where it is and where it leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfLink {
    /// The clickable area, as fractions of the page as displayed.
    pub rect: PageRect,
    /// Where it leads.
    pub target: LinkTarget,
}

/// The links of `page`, in the document's order.
pub fn page_links(doc: &PdfDocument, page: PageIndex) -> Result<Vec<PdfLink>, PdfError> {
    let sheet = doc.page(page)?;
    let (crop, rotation) = (sheet.crop_box(), sheet.rotation());
    let count = doc.page_count().get();
    Ok(sheet
        .page_links()
        .into_iter()
        .map(|link| PdfLink {
            rect: displayed(link.rect, crop, rotation),
            target: leads_to(link.target, count),
        })
        .collect())
}

/// pdfrum's link target as ours. Its enum is non-exhaustive, so what is not a page or an address
/// is `Other` by construction rather than by a wildcard arm.
fn leads_to(target: Target, count: u32) -> LinkTarget {
    if let Target::Page(to) = &target {
        let page = u32::from(*to);
        return if page < count {
            LinkTarget::Page(PageIndex(page))
        } else {
            LinkTarget::Other
        };
    }
    if let Target::Uri(uri) = target {
        return LinkTarget::Uri(uri);
    }
    LinkTarget::Other
}
