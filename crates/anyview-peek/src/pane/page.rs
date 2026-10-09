//! A PDF page across the boundary: `ds_blitz` hands back quire's `PdfPage`, the pane draws quire's
//! `PdfThumb`, and the peek in between speaks its own [`PageLook`].

use crate::pdf::{PageLook, PageTrouble};
use ds::components::content::image_source::{ImageSize, ImageSource};
use ds::components::content::pdf_thumb::{PdfPage, PdfTrouble};

/// What the rasteriser returned, as the peek's own words. A blocking read never answers "still
/// loading"; if it did, the page could not be read.
pub(crate) fn look(page: PdfPage) -> PageLook {
    match page {
        PdfPage::Ready { image, sheet } => PageLook::Drawn {
            source: image.0,
            width: sheet.width,
            height: sheet.height,
        },
        PdfPage::Empty => PageLook::Blank,
        PdfPage::Failed(PdfTrouble::Locked) => PageLook::Failed(PageTrouble::Locked),
        PdfPage::Failed(PdfTrouble::Unreadable) | PdfPage::Loading => {
            PageLook::Failed(PageTrouble::Unreadable)
        }
    }
}

/// What the pane's `PdfThumb` draws for a peeked page.
pub(crate) fn thumb(look: &PageLook) -> PdfPage {
    match look {
        PageLook::Drawn {
            source,
            width,
            height,
        } => PdfPage::Ready {
            image: ImageSource(source.clone()),
            sheet: ImageSize {
                width: *width,
                height: *height,
            },
        },
        PageLook::Blank => PdfPage::Empty,
        PageLook::Failed(PageTrouble::Locked) => PdfPage::Failed(PdfTrouble::Locked),
        PageLook::Failed(PageTrouble::Unreadable) => PdfPage::Failed(PdfTrouble::Unreadable),
    }
}
