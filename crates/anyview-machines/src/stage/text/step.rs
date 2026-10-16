//! The text stage's transitions.

use super::super::find::{FindHits, FindOut, StageFind};
use super::super::zoom::{ContentScale, ZoomDir};
use super::editing::editing;
use super::finding::finding;
use super::model::{
    Editable, Edited, TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep, TextView,
    TextViews, Wrap,
};
use super::steps::stepped as step_line;
use crate::typed::TypedText;
use anyview_core::{LineIndex, Resume};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

pub(super) type Step = (TextStage, Vec<TextOut>);

impl Machine for TextStage {
    type In = TextIn;
    type Out = TextOut;
    type Params = TextParams;
    type Ctx = ();

    fn step(self, input: TextIn, _at: Stamp, params: &TextParams, _cx: &()) -> Step {
        match self {
            TextStage::Reading { place } => reading(place, input, params),
            TextStage::Finding { query, hits, place } => finding(query, hits, place, input, params),
            TextStage::Editing {
                place,
                edited,
                find,
            } => editing(place, edited, find, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TextStage::Reading { place: _ }
            | TextStage::Finding {
                query: _,
                hits: _,
                place: _,
            }
            | TextStage::Editing {
                place: _,
                edited: _,
                find: _,
            } => None,
        }
    }
}

pub(super) fn remember(line: LineIndex) -> TextOut {
    TextOut::Remember(Resume::Text { line })
}

pub(super) fn search(query: &TypedText) -> TextOut {
    TextOut::Find(FindOut::asked(query))
}

fn other_wrap(wrap: Wrap) -> Wrap {
    match wrap {
        Wrap::On => Wrap::Off,
        Wrap::Off => Wrap::On,
    }
}

/// The place with its wrapping switched, and the output that keeps the choice.
pub(super) fn rewrapped(place: TextPlace) -> (TextPlace, Vec<TextOut>) {
    let wrap = other_wrap(place.wrap);
    (TextPlace { wrap, ..place }, vec![TextOut::Wrapped(wrap)])
}

/// The place drawn one step larger or smaller (`Some`), or at actual size (`None`).
pub(super) fn zoomed(place: TextPlace, step: Option<ZoomDir>) -> TextPlace {
    let scale = match step {
        Some(dir) => place.scale.stepped(dir),
        None => ContentScale::ACTUAL,
    };
    TextPlace { scale, ..place }
}

/// Editing starts in the source: the place with that view, and the output that shows it when
/// the view changes.
fn source_for_edit(place: TextPlace) -> (TextPlace, Vec<TextOut>) {
    match place.view {
        TextView::Source => (place, vec![]),
        TextView::Rendered => (
            TextPlace {
                view: TextView::Source,
                ..place
            },
            vec![TextOut::Show(TextView::Source)],
        ),
    }
}

/// The stage that starts editing from `place`, carrying `find` over, and what it asks for.
pub(super) fn begun(place: TextPlace, find: Option<StageFind>, params: &TextParams) -> Step {
    match params.editable {
        Editable::No => match find {
            Some(StageFind { query, hits }) => (TextStage::Finding { query, hits, place }, vec![]),
            None => (TextStage::Reading { place }, vec![]),
        },
        Editable::Yes => {
            let (place, mut outs) = source_for_edit(place);
            outs.push(TextOut::BeginEdit);
            let stage = TextStage::Editing {
                place,
                edited: Edited::default(),
                find,
            };
            (stage, outs)
        }
    }
}

/// The other view, when the file has one.
pub(super) fn other_view(view: TextView, views: TextViews) -> Option<TextView> {
    match (views, view) {
        (TextViews::RenderedAndSource, TextView::Rendered) => Some(TextView::Source),
        (TextViews::RenderedAndSource, TextView::Source) => Some(TextView::Rendered),
        (TextViews::SourceOnly, TextView::Rendered | TextView::Source) => None,
    }
}

/// A key step: the line at the top moves, and the new place is remembered when it did.
pub(super) fn walked(
    place: TextPlace,
    step: TextStep,
    params: &TextParams,
) -> (TextPlace, Vec<TextOut>) {
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
            let outs = vec![search(&query)];
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
        TextIn::ToggleWrap => {
            let (place, outs) = rewrapped(place);
            (TextStage::Reading { place }, outs)
        }
        TextIn::ZoomStep(dir) => stay(zoomed(place, Some(dir))),
        TextIn::ZoomActual => stay(zoomed(place, None)),
        TextIn::Edit => begun(place, None, params),
        TextIn::Restore(Resume::Text { line }) => (
            TextStage::Reading {
                place: TextPlace { line, ..place },
            },
            vec![TextOut::ScrollTo(line)],
        ),
        TextIn::Restore(
            Resume::Raster { .. }
            | Resume::Pdf { .. }
            | Resume::Media { .. }
            | Resume::Book { .. }
            | Resume::Nothing,
        )
        | TextIn::Results {
            query: _,
            count: _,
            nearest: _,
        }
        | TextIn::NextHit
        | TextIn::PreviousHit
        | TextIn::GoToHit(_)
        | TextIn::CloseFind
        | TextIn::Done
        | TextIn::Save
        | TextIn::Edited(_)
        | TextIn::Disk(_)
        | TextIn::Elapsed => stay(place),
    }
}
