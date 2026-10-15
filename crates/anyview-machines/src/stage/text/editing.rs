//! The text stage while the text is being edited.

use super::super::find::{FindHits, FindOut, HitStep, StageFind};
use super::model::{Edited, TextIn, TextOut, TextParams, TextPlace, TextStage};
use super::step::{Step, remember, rewrapped, search, walked, zoomed};

/// The outputs that show the current hit of `hits`, if there is one.
fn shown(hits: FindHits) -> Vec<TextOut> {
    hits.current()
        .map(|hit| TextOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect()
}

/// The editing stage after the current hit of its find moved by `moved`, showing the hit it
/// lands on; with no find up, nothing moves.
fn hit_moved(
    place: TextPlace,
    edited: Edited,
    find: Option<StageFind>,
    moved: impl FnOnce(FindHits) -> FindHits,
) -> Step {
    match find {
        Some(StageFind { query, hits }) => {
            let hits = moved(hits);
            let stage = TextStage::Editing {
                place,
                edited,
                find: Some(StageFind { query, hits }),
            };
            (stage, shown(hits))
        }
        None => (
            TextStage::Editing {
                place,
                edited,
                find: None,
            },
            vec![],
        ),
    }
}

/// Editing. A find over the text is asked again after every change, so its hits follow the
/// typing; only a find the person has just typed moves the view to a hit.
pub(super) fn editing(
    place: TextPlace,
    edited: Edited,
    find: Option<StageFind>,
    input: TextIn,
    params: &TextParams,
) -> Step {
    let keep = |place: TextPlace, edited: Edited, find: Option<StageFind>| TextStage::Editing {
        place,
        edited,
        find,
    };
    let stay = |find: Option<StageFind>| (keep(place, edited, find), vec![]);
    match input {
        TextIn::Scroll(line) => (
            keep(TextPlace { line, ..place }, edited, find),
            vec![remember(line)],
        ),
        TextIn::Step(step) => {
            let (place, outs) = walked(place, step, params);
            (keep(place, edited, find), outs)
        }
        TextIn::Find(query) => {
            let outs = vec![search(&query)];
            let hits = FindHits::asked(&query);
            (keep(place, edited, Some(StageFind { query, hits })), outs)
        }
        TextIn::Results {
            query: answered,
            count,
            nearest,
        } => match find {
            Some(StageFind {
                query,
                hits: before,
            }) if query == answered => {
                let hits = FindHits::answered(count, nearest);
                let outs = match before {
                    FindHits::Pending => shown(hits),
                    FindHits::Idle | FindHits::NoMatch | FindHits::Found(_) => vec![],
                };
                (keep(place, edited, Some(StageFind { query, hits })), outs)
            }
            other => stay(other),
        },
        TextIn::NextHit => hit_moved(place, edited, find, |hits| hits.stepped(HitStep::Next)),
        TextIn::PreviousHit => {
            hit_moved(place, edited, find, |hits| hits.stepped(HitStep::Previous))
        }
        TextIn::GoToHit(hit) => hit_moved(place, edited, find, |hits| hits.jumped(hit)),
        TextIn::CloseFind => match find {
            Some(_) => (
                keep(place, edited, None),
                vec![TextOut::Find(FindOut::Clear)],
            ),
            None => stay(None),
        },
        TextIn::Edited(changes) => {
            let outs = match &find {
                Some(StageFind { query, hits: _ }) if !query.is_empty() => vec![search(query)],
                Some(_) | None => vec![],
            };
            (keep(place, Edited { changes, ..edited }, find), outs)
        }
        TextIn::Disk(outside) => (keep(place, Edited { outside, ..edited }, find), vec![]),
        TextIn::Save => (keep(place, edited, find), vec![TextOut::Save]),
        TextIn::Done => {
            let mut outs = vec![TextOut::EndEdit];
            if find.is_some() {
                outs.push(TextOut::Find(FindOut::Clear));
            }
            (TextStage::Reading { place }, outs)
        }
        TextIn::ToggleWrap => {
            let (place, outs) = rewrapped(place);
            (keep(place, edited, find), outs)
        }
        TextIn::ZoomStep(dir) => (keep(zoomed(place, Some(dir)), edited, find), vec![]),
        TextIn::ZoomActual => (keep(zoomed(place, None), edited, find), vec![]),
        TextIn::Edit | TextIn::ToggleSource | TextIn::Restore(_) | TextIn::Elapsed => stay(find),
    }
}
