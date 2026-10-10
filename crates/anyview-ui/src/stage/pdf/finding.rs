//! The PDF stage while a find is up.

use super::super::find::{FindHits, FindOut, HitIndex, HitStep};
use super::model::{PageView, PdfIn, PdfOut, PdfParams, PdfStage};
use super::step::{Step, page_beside, page_edit, remember, scrolled, search, zoom_asked};
use crate::typed::TypedText;

fn stay(query: TypedText, hits: FindHits, view: PageView) -> Step {
    (PdfStage::Finding { query, hits, view }, vec![])
}

/// Step to the next or previous hit, asking for it to be shown.
fn stepped(query: TypedText, hits: FindHits, view: PageView, step: HitStep) -> Step {
    let hits = hits.stepped(step);
    let outs = hits
        .current()
        .map(|hit| PdfOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect();
    (PdfStage::Finding { query, hits, view }, outs)
}

/// Make `hit` the current one, asking for it to be shown.
fn jumped(query: TypedText, hits: FindHits, view: PageView, hit: HitIndex) -> Step {
    let hits = hits.jumped(hit);
    let outs = hits
        .current()
        .map(|hit| PdfOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect();
    (PdfStage::Finding { query, hits, view }, outs)
}

pub(super) fn finding(
    query: TypedText,
    hits: FindHits,
    view: PageView,
    input: PdfIn,
    params: &PdfParams,
) -> Step {
    match input {
        PdfIn::Scroll { page, offset } => {
            let view = scrolled(view, page, offset, params);
            (
                PdfStage::Finding { query, hits, view },
                vec![remember(view)],
            )
        }
        PdfIn::SetZoom(_) | PdfIn::ZoomStep(_) => match zoom_asked(&input, params) {
            Some(zoom) => {
                let view = PageView { zoom, ..view };
                (
                    PdfStage::Finding { query, hits, view },
                    vec![remember(view)],
                )
            }
            None => stay(query, hits, view),
        },
        PdfIn::Find(text) => {
            let outs = vec![search(&text)];
            let hits = FindHits::asked(&text);
            let state = PdfStage::Finding {
                query: text,
                hits,
                view,
            };
            (state, outs)
        }
        PdfIn::Results {
            query: answered,
            count,
            nearest,
        } if answered == query => {
            let hits = FindHits::answered(count, nearest);
            let outs = hits
                .current()
                .map(|hit| PdfOut::Find(FindOut::ShowHit(hit)))
                .into_iter()
                .collect();
            (PdfStage::Finding { query, hits, view }, outs)
        }
        PdfIn::NextHit => stepped(query, hits, view, HitStep::Next),
        PdfIn::PreviousHit => stepped(query, hits, view, HitStep::Previous),
        PdfIn::GoToHit(hit) => jumped(query, hits, view, hit),
        PdfIn::CloseFind => closed(view),
        PdfIn::GoTo(target) => {
            let outs = vec![PdfOut::ScrollTo(target)];
            (PdfStage::Finding { query, hits, view }, outs)
        }
        PdfIn::DeletePage | PdfIn::MovePage(_) => {
            let outs = page_edit(view.page, &input, params);
            (PdfStage::Finding { query, hits, view }, outs)
        }
        PdfIn::NextPage | PdfIn::PreviousPage => match page_beside(view.page, &input, params) {
            Some(target) => (
                PdfStage::Finding { query, hits, view },
                vec![PdfOut::ScrollTo(target)],
            ),
            None => stay(query, hits, view),
        },
        PdfIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | PdfIn::Arrived
        | PdfIn::Restore(_)
        | PdfIn::Elapsed => stay(query, hits, view),
    }
}

fn closed(view: PageView) -> Step {
    (
        PdfStage::Reading { view },
        vec![PdfOut::Find(FindOut::Clear)],
    )
}
