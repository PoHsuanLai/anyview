//! Where the reader is in a PDF, and the places a key moves them to: a line, the start, the end.

use super::model::{Destination, PdfStage};
use anyview_core::{PageCount, PageIndex, Permille};

/// How far a line key moves, in thousandths of a page: about a line of the text at the fit zoom.
const LINE: u32 = 80;

/// Which way a line key moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineDir {
    /// Towards the start.
    Up,
    /// Towards the end.
    Down,
}

impl PdfStage {
    /// Where the reader is, or is on the way to.
    pub fn place(&self) -> Destination {
        match self {
            PdfStage::Reading { view } | PdfStage::Finding { view, .. } => Destination {
                page: view.page,
                offset: view.offset,
            },
            PdfStage::Jumping { target, view: _ } => *target,
        }
    }
}

/// The place a line key from `from` moves to: the next page begins where this one ends, and the
/// ends of the document stop it.
pub(crate) fn nudged(from: Destination, dir: LineDir, pages: PageCount) -> Destination {
    let whole = Permille::WHOLE.0;
    let (page, offset) = match dir {
        LineDir::Down if from.offset.0 + LINE >= whole => (
            PageIndex(from.page.0.saturating_add(1)),
            from.offset.0 + LINE - whole,
        ),
        LineDir::Down => (from.page, from.offset.0 + LINE),
        LineDir::Up if from.offset.0 >= LINE => (from.page, from.offset.0 - LINE),
        LineDir::Up if from.page.0 == 0 => (from.page, 0),
        LineDir::Up => (PageIndex(from.page.0 - 1), whole + from.offset.0 - LINE),
    };
    let clamped = pages.clamp(page);
    if clamped == page {
        Destination {
            page,
            offset: Permille(offset),
        }
    } else {
        // Past the last page: stay on it, at its foot.
        Destination {
            page: clamped,
            offset: Permille(whole),
        }
    }
}

/// The start of the document.
pub(crate) fn start() -> Destination {
    Destination {
        page: PageIndex(0),
        offset: Permille(0),
    }
}

/// The top of the last page.
pub(crate) fn end(pages: PageCount) -> Destination {
    Destination {
        page: pages.last(),
        offset: Permille(0),
    }
}
