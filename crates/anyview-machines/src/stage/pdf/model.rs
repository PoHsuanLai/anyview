//! The PDF stage's states, inputs and outputs.

use super::super::find::{FindHits, FindOut, HitCount, HitIndex};
use super::super::media::StepDirection;
use super::super::zoom::{Viewport, ZoomDir};
use crate::edits::EditRequest;
use crate::typed::TypedText;
use anyview_core::{PageCount, PageIndex, Permille, Resume, Zoom};

/// Where in the document the view is: a page, how far down it (in thousandths of its height) and
/// the zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageView {
    /// The page at the top of the view.
    pub page: PageIndex,
    /// How far down that page the view starts.
    pub offset: Permille,
    /// How large pages are drawn.
    pub zoom: Zoom,
}

/// A place to go: a page and how far down it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination {
    /// The page.
    pub page: PageIndex,
    /// How far down it, in thousandths of its height.
    pub offset: Permille,
}

/// What the PDF stage is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfStage {
    /// Reading where `view` is.
    Reading { view: PageView },
    /// A find is up: `hits` says where its search stands, and `view` is where the reader is.
    Finding {
        query: TypedText,
        hits: FindHits,
        view: PageView,
    },
    /// A scroll to `target` is under way (a link, a page key, the outline); it ends with
    /// `Arrived`, or when the reader scrolls.
    Jumping { target: Destination, view: PageView },
}

impl Default for PdfStage {
    fn default() -> Self {
        PdfStage::Reading {
            view: PageView {
                page: PageIndex(0),
                offset: Permille(0),
                zoom: Zoom::Fit,
            },
        }
    }
}

/// What moves the stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfIn {
    /// The reader scrolled: this is the page at the top and how far down it.
    Scroll { page: PageIndex, offset: Permille },
    /// Zoom to this.
    SetZoom(Zoom),
    /// One zoom step in or out.
    ZoomStep(ZoomDir),
    /// Search for this text; empty text closes the find.
    Find(TypedText),
    /// The search for `query` found `count` hits, `nearest` being the one closest to the reader.
    Results {
        query: TypedText,
        count: HitCount,
        nearest: HitIndex,
    },
    /// The next hit, wrapping to the first after the last.
    NextHit,
    /// The previous hit, wrapping to the last before the first.
    PreviousHit,
    /// Make this hit the current one and show it (a row of the palette's hits).
    GoToHit(HitIndex),
    /// Close the find.
    CloseFind,
    /// Go to a place (a link, the outline, a thumbnail).
    GoTo(Destination),
    /// The next page.
    NextPage,
    /// The previous page.
    PreviousPage,
    /// The scroll to the target of a jump finished.
    Arrived,
    /// Remove the page the reader is on.
    DeletePage,
    /// Move the page the reader is on one place earlier or later.
    MovePage(StepDirection),
    /// Restore where the person left the file.
    Restore(Resume),
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for PdfIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        PdfIn::Elapsed
    }
}

/// What the stage wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfOut {
    /// Keep where the person is, for next time.
    Remember(Resume),
    /// Scroll the view here.
    ScrollTo(Destination),
    /// Something for the search: run it, show a hit, clear the marks.
    Find(FindOut),
    /// Change the document's pages: the host saves it.
    Edit(EditRequest),
}

/// What the stage needs to know of the open document and the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfParams {
    /// How many pages the document has.
    pub pages: PageCount,
    /// The scale on screen and the fit scale.
    pub viewport: Viewport,
    /// A zoom step's factor in thousandths (setting `viewer.zoom.step`).
    pub step: Permille,
    /// Whether the pages can be edited.
    pub page_edits: PageEdits,
}

/// Whether the pages of the open PDF take an edit: the pages of a book do not, since it is a PDF
/// only in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageEdits {
    /// Pages can be deleted and moved.
    #[default]
    Allowed,
    /// The pages stay as they are.
    Barred,
}

impl Default for PdfParams {
    fn default() -> Self {
        PdfParams {
            pages: PageCount::new(1).unwrap_or_else(|| unreachable!("one is not zero")),
            viewport: Viewport {
                shown: Permille::WHOLE,
                fit: Permille::WHOLE,
            },
            step: Permille(1250),
            page_edits: PageEdits::Allowed,
        }
    }
}
