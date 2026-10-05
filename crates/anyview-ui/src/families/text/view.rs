//! Text on screen: a clipped room of highlighted lines the worker read, or the Markdown page in
//! a sealed frame. The room owns the wheel (a window of lines scrolls by the line index, not by a
//! native scroll), so the machine's `Scroll(line)` is the one place the reader's position lives.

use super::bar::TextFinding;
use super::doc::TextDoc;
use super::find::{FoundHits, Mark, pieces};
use super::frame;
use crate::families::view::{Held, StageCx};
use crate::{FindHits, Stage, StageIn, TextIn, TextPlace, TextStage, TextView as Shown, Wrap};
use anyview_core::LineIndex;
use anyview_text::{FindHit, TOKEN_CLASS_PREFIX, TokenClass, TokenLine};
use dioxus::prelude::*;
use ds::host::gesture::{Gesture, use_gestures};
use ds::prelude::Word;

/// The height of one line in logical pixels: the `--s-18` step of the stylesheet.
const ROW: f32 = 18.0;

/// Lines asked for beyond the room, so a short scroll lands on lines already read.
const OVERSCAN: u32 = 8;

fn place_of(stage: &Stage) -> Option<TextPlace> {
    match stage {
        Stage::Text(text) => Some(place_of_text(text)),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}

/// Where the text stage is.
pub(super) fn place_of_text(stage: &TextStage) -> TextPlace {
    match stage {
        TextStage::Reading { place } | TextStage::Finding { place, .. } => *place,
    }
}

/// The find the stage has up: its text and where its search stands.
fn find_of(stage: &Stage) -> Option<(crate::TypedText, FindHits)> {
    match stage {
        Stage::Text(TextStage::Finding { query, hits, .. }) => Some((query.clone(), *hits)),
        Stage::Text(TextStage::Reading { .. })
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}

/// How many lines the room holds.
pub(super) fn rows_of(height: f32) -> u32 {
    // A positive, finite height over a constant: truncating to whole rows is the point.
    (height / ROW).floor().max(1.0) as u32
}

fn class_of(class: TokenClass) -> String {
    format!("{TOKEN_CLASS_PREFIX}{}", class.slug())
}

/// The class a piece of a line wears: its token class, and a mark when it is part of a hit.
fn piece_class(class: TokenClass, mark: Mark) -> String {
    match mark {
        Mark::Plain => class_of(class),
        Mark::Hit => format!("{} viewer-hit", class_of(class)),
        Mark::Current => format!("{} viewer-hit viewer-hit-current", class_of(class)),
    }
}

/// One line: its number and its text, cut where the hits on it begin and end. `hits` are the
/// ones on this line and `current` the position among them of the hit the reader is on.
fn line(line: &TokenLine, wrap: Wrap, hits: &[FindHit], current: Option<usize>) -> Element {
    let number = line.number.0 + 1;
    let cut = pieces(line, hits, current);
    rsx! {
        div { class: "viewer-line", "data-wrap": wrap.slug(),
            span { class: "viewer-lineno", "{number}" }
            span { class: "viewer-code",
                for piece in cut.iter() {
                    span { class: piece_class(piece.class, piece.mark), "{piece.text}" }
                }
            }
        }
    }
}

/// The hits on `line` and the position among them of the current hit (`current`, among all).
fn hits_on(
    found: Option<&Held<FoundHits>>,
    line: LineIndex,
    current: Option<crate::HitIndex>,
) -> (&[FindHit], Option<usize>) {
    let Some(found) = found else {
        return (&[], None);
    };
    let (first, hits) = found.0.on_line(line);
    let within = current
        .and_then(|current| current.0.checked_sub(first.0))
        .map(|at| at as usize)
        .filter(|at| *at < hits.len());
    (hits, within)
}

#[component]
pub(super) fn TextContent(doc: Held<TextDoc>, cx: StageCx) -> Element {
    let mut carry = use_signal(|| 0.0_f32);
    let send = cx.send;
    let count = doc.0.line_count().0;
    let scrolled = cx.clone();
    use_gestures(move |gesture| {
        let (Some(area), Some(place)) = (scrolled.area, place_of(&scrolled.stage)) else {
            return;
        };
        let Gesture::Scroll { by, at, .. } = gesture else {
            return;
        };
        let (x, y) = (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0);
        if x < 0.0 || y < 0.0 || x >= area.size.width.0 || y >= area.size.height.0 {
            return;
        }
        // The content moves with the fingers: down means earlier lines.
        let total = carry() - by.y.0;
        let lines = (total / ROW).trunc();
        carry.set(total - lines * ROW);
        if lines != 0.0 {
            let last = count.saturating_sub(1);
            let line = (i64::from(place.line.0) + lines as i64).clamp(0, i64::from(last));
            send.call(StageIn::Text(TextIn::Scroll(LineIndex(
                u32::try_from(line).unwrap_or(0),
            ))));
        }
    });

    let (Some(area), Some(place)) = (cx.area, place_of(&cx.stage)) else {
        return rsx! { div { class: "viewer-text" } };
    };
    let rows = rows_of(area.size.height.0);
    let first = place.line;
    let covered = cx.lines.as_ref().is_some_and(|window| {
        window
            .0
            .covers(first.0..first.0.saturating_add(rows).min(count))
    });
    let ask = cx.ask_lines;
    use_effect(use_reactive!(|first, rows, covered| {
        if !covered {
            ask.call((first, rows + OVERSCAN));
        }
    }));

    if let (Shown::Rendered, Some(rendered)) = (place.view, doc.0.rendered.as_ref()) {
        let page = frame::document(rendered, &cx.frame);
        return rsx! {
            div { class: "viewer-text", "data-view": "rendered",
                iframe { class: "viewer-frame", "data-frame-tag": "page", srcdoc: page }
            }
        };
    }
    let find = find_of(&cx.stage);
    let current = find.as_ref().and_then(|(_, hits)| hits.current());
    rsx! {
        div { class: "viewer-text", "data-view": "source",
            if let Some(window) = cx.lines.as_ref() {
                for visible in window.0.lines.iter().filter(|line| {
                    line.number.0 >= first.0 && line.number.0 < first.0.saturating_add(rows)
                }) {
                    {
                        let (hits, within) = hits_on(cx.hits.as_ref(), visible.number, current);
                        line(visible, place.wrap, hits, within)
                    }
                }
            }
            if let Some((query, hits)) = find {
                TextFinding { query, hits, cx: cx.clone() }
            }
        }
    }
}
