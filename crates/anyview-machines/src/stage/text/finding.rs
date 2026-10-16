//! The text stage while a find is up.

use super::super::find::{FindHits, FindOut, HitIndex, HitStep, StageFind};
use super::model::{TextIn, TextOut, TextParams, TextPlace, TextStage};
use super::step::{Step, begun, other_view, remember, rewrapped, search, walked, zoomed};
use crate::typed::TypedText;

fn stepped(query: TypedText, hits: FindHits, place: TextPlace, step: HitStep) -> Step {
    let hits = hits.stepped(step);
    let outs = hits
        .current()
        .map(|hit| TextOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect();
    (TextStage::Finding { query, hits, place }, outs)
}

/// Make `hit` the current one, asking for it to be shown.
fn jumped(query: TypedText, hits: FindHits, place: TextPlace, hit: HitIndex) -> Step {
    let hits = hits.jumped(hit);
    let outs = hits
        .current()
        .map(|hit| TextOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect();
    (TextStage::Finding { query, hits, place }, outs)
}

fn closed(place: TextPlace) -> Step {
    (
        TextStage::Reading { place },
        vec![TextOut::Find(FindOut::Clear)],
    )
}

pub(super) fn finding(
    query: TypedText,
    hits: FindHits,
    place: TextPlace,
    input: TextIn,
    params: &TextParams,
) -> Step {
    let stay = |query: TypedText, hits: FindHits, place: TextPlace| {
        (TextStage::Finding { query, hits, place }, vec![])
    };
    match input {
        TextIn::Scroll(line) => (
            TextStage::Finding {
                query,
                hits,
                place: TextPlace { line, ..place },
            },
            vec![remember(line)],
        ),
        TextIn::Step(step) => {
            let (place, outs) = walked(place, step, params);
            (TextStage::Finding { query, hits, place }, outs)
        }
        TextIn::Find(text) => {
            let outs = vec![search(&text)];
            let hits = FindHits::asked(&text);
            let state = TextStage::Finding {
                query: text,
                hits,
                place,
            };
            (state, outs)
        }
        TextIn::Results {
            query: answered,
            count,
            nearest,
        } if answered == query => {
            let hits = FindHits::answered(count, nearest);
            let outs = hits
                .current()
                .map(|hit| TextOut::Find(FindOut::ShowHit(hit)))
                .into_iter()
                .collect();
            (TextStage::Finding { query, hits, place }, outs)
        }
        TextIn::NextHit => stepped(query, hits, place, HitStep::Next),
        TextIn::PreviousHit => stepped(query, hits, place, HitStep::Previous),
        TextIn::GoToHit(hit) => jumped(query, hits, place, hit),
        TextIn::CloseFind => closed(place),
        TextIn::ToggleSource => match other_view(place.view, params.views) {
            Some(view) => {
                let outs = vec![TextOut::Show(view), search(&query)];
                let place = TextPlace { view, ..place };
                let hits = FindHits::Pending;
                (TextStage::Finding { query, hits, place }, outs)
            }
            None => stay(query, hits, place),
        },
        TextIn::ToggleWrap => {
            let (place, outs) = rewrapped(place);
            (TextStage::Finding { query, hits, place }, outs)
        }
        TextIn::ZoomStep(dir) => stay(query, hits, zoomed(place, Some(dir))),
        TextIn::ZoomActual => stay(query, hits, zoomed(place, None)),
        TextIn::Edit => begun(place, Some(StageFind { query, hits }), params),
        TextIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | TextIn::Done
        | TextIn::Save
        | TextIn::Edited(_)
        | TextIn::Disk(_)
        | TextIn::Restore(_)
        | TextIn::Elapsed => stay(query, hits, place),
    }
}
