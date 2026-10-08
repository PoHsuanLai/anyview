//! PDF: the PDF stage's view. A document is opened once on a worker (`doc`); the pages are a stack
//! laid out at the scale on screen and placed by pure arithmetic (`scene`); the tiles of the pages in
//! view are drawn and uploaded by workers (`work`), held under a budget (`cache`, `live`) and drawn
//! as `TextureLayer`s (`view`, `page`, `draw`); and the find bar, the capsule and the side panel's
//! thumbnails and outline are drawn from the stage machine's state.

mod bound;
mod cache;
mod capsule;
mod doc;
mod draw;
mod find;
mod live;
#[cfg(test)]
mod markup;
mod page;
mod panel;
mod scene;
mod shelf;
mod steer;
#[cfg(test)]
mod tests;
mod view;
mod work;

pub use doc::{PdfDoc, PdfFailure};
pub use shelf::{PdfShelf, use_pdf_shelf};
pub use work::{Finish, FlightId, PdfAnswer, PdfAsk, PdfTask, ReadyTile};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{NaturalSize, OpenError, OpenLink};
use crate::{
    PanelTab, PanelTabs, PdfParams, Stage, StageFamily, StageIn, StageParams, Ticket, Viewport,
};
use anyview_core::{Facts, FormatKind, PixelLen, PixelSize, Resume, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::priority::RankedSlot;
use std::sync::Arc;

/// PDF documents.
#[derive(Debug, Clone, Copy)]
pub struct PdfStageView;

impl StageView for PdfStageView {
    const FAMILY: StageFamily = StageFamily::Pdf;
    type Doc = PdfDoc;

    fn open(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        _link: &OpenLink,
    ) -> Result<PdfDoc, OpenError> {
        if sniffed.kind() == FormatKind::Book {
            doc::open_book(src, sniffed)
        } else {
            doc::open(src)
        }
    }

    fn facts(doc: &PdfDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(doc: &PdfDoc) -> PanelTabs {
        if doc.outline.is_empty() {
            PanelTabs::of(&[PanelTab::Thumbnails, PanelTab::Info])
        } else {
            PanelTabs::of(&[PanelTab::Thumbnails, PanelTab::Contents, PanelTab::Info])
        }
    }

    fn params(doc: &PdfDoc, stage: &Stage, area: Option<Area>) -> StageParams {
        let mut pdf = PdfParams {
            pages: doc.pages(),
            edits: doc.offer != crate::EditOffer::Withheld,
            ..PdfParams::default()
        };
        if let (Some(view), Some(area)) = (live::page_view(stage), doc::room_of(area)) {
            let fit = scene::fits(doc.sizes(), scene::Frame::of(area));
            pdf.viewport = Viewport {
                shown: scene::scale_of(view.zoom, fit),
                fit: fit.page,
            };
        }
        StageParams {
            pdf,
            ..StageParams::default()
        }
    }

    /// The place the file was left is put back now that the page count is known: before it, the
    /// stage could only keep it inside a one-page document.
    fn natural(doc: &PdfDoc) -> Option<NaturalSize> {
        // The first page as displayed (crop box, after its rotation) at 100%: a point is a pixel.
        let first = doc.sizes().first()?;
        let whole = |points: anyview_pdf::MilliPoints| {
            Some(points.0.saturating_add(500) / 1000).filter(|px| *px > 0)
        };
        Some(NaturalSize::Points(PixelSize {
            width: PixelLen(whole(first.width)?),
            height: PixelLen(whole(first.height)?),
        }))
    }

    fn arrived(_doc: &PdfDoc, stage: &Stage, left_at: &Resume) -> Vec<StageIn> {
        stage.restoring(left_at).into_iter().collect()
    }

    fn stage(doc: &Arc<PdfDoc>, cx: &StageCx) -> Element {
        rsx! { view::PdfContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn edit_offer(doc: &PdfDoc) -> crate::EditOffer {
        doc.offer
    }

    fn slots(doc: &PdfDoc, cx: &StageCx) -> Vec<RankedSlot<crate::Command>> {
        capsule::slots(doc, cx)
    }

    fn panel(doc: &Arc<PdfDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        let doc = Held(Arc::clone(doc));
        match tab {
            PanelTab::Thumbnails => Some(rsx! { panel::Thumbnails { doc, cx: cx.clone() } }),
            PanelTab::Contents => Some(rsx! { panel::Outline { doc, cx: cx.clone() } }),
            PanelTab::Info | PanelTab::Tracks => None,
        }
    }
}
