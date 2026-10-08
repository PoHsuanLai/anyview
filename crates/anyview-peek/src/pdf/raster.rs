//! The peek of a PDF with the `pane` feature: its first page, rasterised by `ds-blitz`'s thumbnail cache.
//!
//! pdfrum is reached only through `ds_blitz::pdf_thumb_blocking` (the `pdf` feature), the same
//! worker-side call and the same cache the launcher's `PdfFileThumb` uses, so a page peeked here
//! is a cache hit when the pane asks for it again at the same size.

use super::PdfPeeked;
use crate::error::PeekError;
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FormatKind, Input, Peek, PeekBudget, Sniffed,
};
use ds::components::content::pdf_thumb::{PdfPage, PdfTrouble};
use ds::components::lists::preview::content::PANE_MEDIA;
use ds::prelude::{Scale, Size, Word};
use ds_blitz::{ThumbRequest, pdf_thumb_blocking, pdf_thumb_bytes};

/// The device pixels a page is rasterised for at 2x: the pane's media box twice over.
const SHARP_AREA: u64 = 656 * 440;

/// The peek of the kind `Pdf`.
///
/// `ds-blitz` reads the whole file, so a file longer than the budget's bytes is refused with
/// [`PeekError::OverBudget`] rather than read past it. The page is fitted into the pane's media
/// box at 2x, or at 1x when the budget's pixels do not cover that.
#[derive(Debug, Clone, Copy)]
pub struct PdfPeek;

/// The scale a page is rasterised at: sharp for a budget that allows it.
fn scale_for(budget: &PeekBudget) -> Scale {
    if budget.pixels.0 >= SHARP_AREA {
        Scale(240)
    } else {
        Scale::ONE
    }
}

impl Peek for PdfPeek {
    const KIND: FormatKind = FormatKind::Pdf;
    type Peeked = PdfPeeked;
    type Error = PeekError;

    fn peek(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<PdfPeeked, PeekError> {
        if sniffed.kind() != Self::KIND {
            return Err(PeekError::WrongKind {
                kind: sniffed.kind(),
            });
        }
        let len = ByteLen(src.stamp().len.0.max(src.bytes().len().0));
        if len > budget.bytes {
            return Err(PeekError::OverBudget {
                len,
                allowed: budget.bytes,
            });
        }
        let room: Size = PANE_MEDIA;
        let request = ThumbRequest {
            path: src.label(),
            size: room,
            scale: scale_for(budget),
        };
        // A file on disk goes through the page cache, which a pane asks again at the same size;
        // bytes handed in are rasterised from memory, whole, as the budget allows.
        let page = match src.path() {
            Some(_) => pdf_thumb_blocking(&request),
            None => match src.bytes().read_range(0..budget.bytes.0) {
                Ok(bytes) => pdf_thumb_bytes(bytes, request.device_box()),
                Err(_) => PdfPage::Failed(PdfTrouble::Unreadable),
            },
        };
        Ok(PdfPeeked { page })
    }

    fn facts(peeked: &PdfPeeked) -> Facts {
        let kind = |text: &str| Facts::empty().with(FactLabel::Kind, FactValue::text(text));
        match &peeked.page {
            PdfPage::Ready { sheet, .. } => kind("PDF document").with(
                FactLabel::Dimensions,
                FactValue::text(format!("{} × {} pt", sheet.width, sheet.height)),
            ),
            PdfPage::Empty => {
                kind("PDF document").with(FactLabel::Pages, FactValue::text("No pages"))
            }
            PdfPage::Failed(trouble) => kind(trouble.label()),
            // A blocking read never answers "still loading"; the arm keeps the match whole.
            PdfPage::Loading => kind(PdfTrouble::Unreadable.label()),
        }
    }
}
