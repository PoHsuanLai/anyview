//! The text stage's transitions.

use super::super::find::{FindHits, FindOut, HitStep};
use super::model::{
    TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep, TextView, TextViews, Wrap,
};
use super::steps::stepped as step_line;
use crate::typed::TypedText;
use anyview_core::{LineIndex, Resume};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (TextStage, Vec<TextOut>);

impl Machine for TextStage {
    type In = TextIn;
    type Out = TextOut;
    type Params = TextParams;
    type Ctx = ();

    fn step(self, input: TextIn, _at: Stamp, params: &TextParams, _cx: &()) -> Step {
        match self {
            TextStage::Reading { place } => reading(place, input, params),
            TextStage::Finding { query, hits, place } => finding(query, hits, place, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TextStage::Reading { place: _ }
            | TextStage::Finding {
                query: _,
                hits: _,
                place: _,
            } => None,
        }
    }
}

fn remember(line: LineIndex) -> TextOut {
    TextOut::Remember(Resume::Text { line })
}

fn search(query: &TypedText) -> TextOut {
    TextOut::Find(FindOut::asked(query))
}

fn other_wrap(wrap: Wrap) -> Wrap {
    match wrap {
        Wrap::On => Wrap::Off,
        Wrap::Off => Wrap::On,
    }
}

/// The other view, when the file has one.
fn other_view(view: TextView, views: TextViews) -> Option<TextView> {
    match (views, view) {
        (TextViews::RenderedAndSource, TextView::Rendered) => Some(TextView::Source),
        (TextViews::RenderedAndSource, TextView::Source) => Some(TextView::Rendered),
        (TextViews::SourceOnly, TextView::Rendered | TextView::Source) => None,
    }
}

/// A find marks its hits in the source lines, which a rendered page does not have: when the file
/// has a source view, the find starts there.
fn source_for_hits(place: TextPlace, params: &TextParams) -> (TextPlace, Vec<TextOut>) {
    match (place.view, params.views) {
        (TextView::Rendered, TextViews::RenderedAndSource) => (
            TextPlace {
                view: TextView::Source,
                ..place
            },
            vec![TextOut::Show(TextView::Source)],
        ),
        (TextView::Rendered, TextViews::SourceOnly)
        | (TextView::Source, TextViews::RenderedAndSource | TextViews::SourceOnly) => {
            (place, vec![])
        }
    }
}

/// A key step: the line at the top moves, and the new place is remembered when it did.
fn walked(place: TextPlace, step: TextStep, params: &TextParams) -> (TextPlace, Vec<TextOut>) {
    let line = step_line(place.line, step, params.extent);
    if line == place.line {
        (place, vec![])
    } else {
        (TextPlace { line, ..place }, vec![remember(line)])
    }
}

fn reading(place: TextPlace, input: TextIn, params: &TextParams) -> Step {
    let stay = |place: TextPlace| (TextStage::Reading { place }, vec![]);
    match input {
        TextIn::Scroll(line) => (
            TextStage::Reading {
                place: TextPlace { line, ..place },
            },
            vec![remember(line)],
        ),
        TextIn::Step(step) => {
            let (place, outs) = walked(place, step, params);
            (TextStage::Reading { place }, outs)
        }
        TextIn::Find(query) => {
            let (place, mut outs) = source_for_hits(place, params);
            outs.push(search(&query));
            let hits = FindHits::asked(&query);
            (TextStage::Finding { query, hits, place }, outs)
        }
        TextIn::ToggleSource => match other_view(place.view, params.views) {
            Some(view) => (
                TextStage::Reading {
                    place: TextPlace { view, ..place },
                },
                vec![TextOut::Show(view)],
            ),
            None => stay(place),
        },
        TextIn::ToggleWrap => stay(TextPlace {
            wrap: other_wrap(place.wrap),
            ..place
        }),
        TextIn::Restore(Resume::Text { line }) => (
            TextStage::Reading {
                place: TextPlace { line, ..place },
            },
            vec![TextOut::ScrollTo(line)],
        ),
        TextIn::Restore(
            Resume::Raster { .. } | Resume::Pdf { .. } | Resume::Media { .. } | Resume::Nothing,
        )
        | TextIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | TextIn::NextHit
        | TextIn::PreviousHit
        | TextIn::CloseFind
        | TextIn::Elapsed => stay(place),
    }
}

fn stepped(query: TypedText, hits: FindHits, place: TextPlace, step: HitStep) -> Step {
    let hits = hits.stepped(step);
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

fn finding(
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
            let place = TextPlace {
                wrap: other_wrap(place.wrap),
                ..place
            };
            stay(query, hits, place)
        }
        TextIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | TextIn::Restore(_)
        | TextIn::Elapsed => stay(query, hits, place),
    }
}
