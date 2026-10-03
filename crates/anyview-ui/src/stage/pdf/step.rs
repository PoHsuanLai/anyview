//! The PDF stage's transitions for reading and jumping; finding is in `finding`.

use super::super::find::{FindHits, FindOut};
use super::super::zoom::stepped;
use super::finding::finding;
use super::model::{Destination, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
use crate::typed::TypedText;
use anyview_core::{PageIndex, Permille, Resume, Zoom};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

pub(super) type Step = (PdfStage, Vec<PdfOut>);

impl Machine for PdfStage {
    type In = PdfIn;
    type Out = PdfOut;
    type Params = PdfParams;

    fn step(self, input: PdfIn, _at: Stamp, params: &PdfParams) -> Step {
        match self {
            PdfStage::Reading { view } => reading(view, input, params),
            PdfStage::Finding { query, hits, view } => finding(query, hits, view, input, params),
            PdfStage::Jumping { target, view } => jumping(target, view, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            PdfStage::Reading { view: _ }
            | PdfStage::Finding {
                query: _,
                hits: _,
                view: _,
            }
            | PdfStage::Jumping { target: _, view: _ } => None,
        }
    }
}

pub(super) fn remember(view: PageView) -> PdfOut {
    PdfOut::Remember(Resume::Pdf {
        page: view.page,
        offset: view.offset,
        zoom: view.zoom,
    })
}

/// The view after the reader scrolled to `page` and `offset`, kept inside the document.
pub(super) fn scrolled(
    view: PageView,
    page: PageIndex,
    offset: Permille,
    params: &PdfParams,
) -> PageView {
    PageView {
        page: params.pages.clamp(page),
        offset: Permille(offset.0.min(Permille::WHOLE.0)),
        ..view
    }
}

/// The zoom a zoom input asks for, kept inside the limits; `None` for any other input.
pub(super) fn zoom_asked(input: &PdfIn, params: &PdfParams) -> Option<Zoom> {
    match input {
        PdfIn::SetZoom(Zoom::Scale(scale)) => Some(Zoom::scaled(*scale)),
        PdfIn::SetZoom(zoom) => Some(*zoom),
        PdfIn::ZoomStep(dir) => Some(stepped(params.viewport, *dir, params.step)),
        PdfIn::Scroll { page: _, offset: _ }
        | PdfIn::Find(_)
        | PdfIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | PdfIn::NextHit
        | PdfIn::PreviousHit
        | PdfIn::CloseFind
        | PdfIn::GoTo(_)
        | PdfIn::NextPage
        | PdfIn::PreviousPage
        | PdfIn::Arrived
        | PdfIn::Restore(_)
        | PdfIn::Elapsed => None,
    }
}

/// The destination one page from `page`, or `None` at the end of the document.
pub(super) fn page_beside(
    page: PageIndex,
    input: &PdfIn,
    params: &PdfParams,
) -> Option<Destination> {
    let target = match input {
        PdfIn::NextPage => PageIndex(page.0.saturating_add(1)),
        PdfIn::PreviousPage => PageIndex(page.0.saturating_sub(1)),
        PdfIn::Scroll { page: _, offset: _ }
        | PdfIn::SetZoom(_)
        | PdfIn::ZoomStep(_)
        | PdfIn::Find(_)
        | PdfIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | PdfIn::NextHit
        | PdfIn::PreviousHit
        | PdfIn::CloseFind
        | PdfIn::GoTo(_)
        | PdfIn::Arrived
        | PdfIn::Restore(_)
        | PdfIn::Elapsed => return None,
    };
    (params.pages.clamp(target) != page).then_some(Destination {
        page: params.pages.clamp(target),
        offset: Permille(0),
    })
}

pub(super) fn search(query: &TypedText) -> PdfOut {
    PdfOut::Find(FindOut::asked(query))
}

fn reading(view: PageView, input: PdfIn, params: &PdfParams) -> Step {
    let stay = |view: PageView| (PdfStage::Reading { view }, vec![]);
    match input {
        PdfIn::Scroll { page, offset } => {
            let view = scrolled(view, page, offset, params);
            (PdfStage::Reading { view }, vec![remember(view)])
        }
        PdfIn::SetZoom(_) | PdfIn::ZoomStep(_) => match zoom_asked(&input, params) {
            Some(zoom) => {
                let view = PageView { zoom, ..view };
                (PdfStage::Reading { view }, vec![remember(view)])
            }
            None => stay(view),
        },
        PdfIn::Find(query) => {
            let outs = vec![search(&query)];
            let hits = FindHits::asked(&query);
            (PdfStage::Finding { query, hits, view }, outs)
        }
        PdfIn::GoTo(target) => jump(view, target, params),
        PdfIn::NextPage | PdfIn::PreviousPage => match page_beside(view.page, &input, params) {
            Some(target) => jump(view, target, params),
            None => stay(view),
        },
        PdfIn::Restore(Resume::Pdf { page, offset, zoom }) => {
            let view = scrolled(
                PageView {
                    zoom: zoom_loaded(zoom),
                    ..view
                },
                page,
                offset,
                params,
            );
            let to = Destination {
                page: view.page,
                offset: view.offset,
            };
            (PdfStage::Reading { view }, vec![PdfOut::ScrollTo(to)])
        }
        PdfIn::Restore(
            Resume::Raster { .. } | Resume::Media { .. } | Resume::Text { .. } | Resume::Nothing,
        )
        | PdfIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | PdfIn::NextHit
        | PdfIn::PreviousHit
        | PdfIn::CloseFind
        | PdfIn::Arrived
        | PdfIn::Elapsed => stay(view),
    }
}

/// A stored zoom, clamped: a stored `Scale` loads as written.
fn zoom_loaded(zoom: Zoom) -> Zoom {
    match zoom {
        Zoom::Scale(scale) => Zoom::scaled(scale),
        Zoom::Fit | Zoom::Fill | Zoom::Actual => zoom,
    }
}

/// A jump from `view` to `target`, the page kept inside the document.
fn jump(view: PageView, target: Destination, params: &PdfParams) -> Step {
    let target = Destination {
        page: params.pages.clamp(target.page),
        offset: Permille(target.offset.0.min(Permille::WHOLE.0)),
    };
    (
        PdfStage::Jumping { target, view },
        vec![PdfOut::ScrollTo(target)],
    )
}

fn jumping(target: Destination, view: PageView, input: PdfIn, params: &PdfParams) -> Step {
    let this = PdfStage::Jumping { target, view };
    match input {
        PdfIn::Arrived => {
            let view = PageView {
                page: target.page,
                offset: target.offset,
                ..view
            };
            (PdfStage::Reading { view }, vec![remember(view)])
        }
        PdfIn::Scroll { page, offset } => {
            let view = scrolled(view, page, offset, params);
            (PdfStage::Reading { view }, vec![remember(view)])
        }
        PdfIn::SetZoom(_) | PdfIn::ZoomStep(_) => match zoom_asked(&input, params) {
            Some(zoom) => (
                PdfStage::Jumping {
                    target,
                    view: PageView { zoom, ..view },
                },
                vec![],
            ),
            None => (this, vec![]),
        },
        PdfIn::GoTo(to) => jump(view, to, params),
        PdfIn::NextPage | PdfIn::PreviousPage => match page_beside(target.page, &input, params) {
            Some(to) => jump(view, to, params),
            None => (this, vec![]),
        },
        PdfIn::Find(query) => {
            let view = PageView {
                page: target.page,
                offset: target.offset,
                ..view
            };
            let outs = vec![search(&query)];
            let hits = FindHits::asked(&query);
            (PdfStage::Finding { query, hits, view }, outs)
        }
        PdfIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | PdfIn::NextHit
        | PdfIn::PreviousHit
        | PdfIn::CloseFind
        | PdfIn::Restore(_)
        | PdfIn::Elapsed => (this, vec![]),
    }
}
