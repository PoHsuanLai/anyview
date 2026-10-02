//! Text on screen: a clipped room of highlighted lines the worker read, or the Markdown page in
//! a sealed frame. The room owns the wheel (a window of lines scrolls by the line index, not by a
//! native scroll), so the machine's `Scroll(line)` is the one place the reader's position lives.

use super::doc::TextDoc;
use super::frame;
use crate::families::view::{Held, StageCx};
use crate::{Stage, StageIn, TextIn, TextPlace, TextStage, TextView as Shown, Wrap};
use anyview_core::LineIndex;
use anyview_text::{TOKEN_CLASS_PREFIX, TokenClass, TokenLine};
use dioxus::prelude::*;
use ds::host::gesture::{Gesture, use_gestures};
use ds::prelude::Word;

/// The height of one line in logical pixels: the `--s-18` step of the stylesheet.
const ROW: f32 = 18.0;

/// Lines asked for beyond the room, so a short scroll lands on lines already read.
const OVERSCAN: u32 = 8;

fn place_of(stage: &Stage) -> Option<TextPlace> {
    match stage {
        Stage::Text(TextStage::Reading { place } | TextStage::Finding { place, .. }) => {
            Some(*place)
        }
        Stage::NoStage | Stage::Raster(_) | Stage::Pdf(_) | Stage::Media(_) => None,
    }
}

/// How many lines the room holds.
fn rows_of(height: f32) -> u32 {
    // A positive, finite height over a constant: truncating to whole rows is the point.
    (height / ROW).floor().max(1.0) as u32
}

fn class_of(class: TokenClass) -> String {
    format!("{TOKEN_CLASS_PREFIX}{}", class.slug())
}

fn line(line: &TokenLine, wrap: Wrap) -> Element {
    let number = line.number.0 + 1;
    rsx! {
        div { class: "viewer-line", "data-wrap": wrap.slug(),
            span { class: "viewer-lineno", "{number}" }
            span { class: "viewer-code",
                for span in line.spans.iter() {
                    span { class: class_of(span.class), "{span.text}" }
                }
            }
        }
    }
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
    rsx! {
        div { class: "viewer-text", "data-view": "source",
            if let Some(window) = cx.lines.as_ref() {
                for visible in window.0.lines.iter().filter(|line| {
                    line.number.0 >= first.0 && line.number.0 < first.0.saturating_add(rows)
                }) {
                    {line(visible, place.wrap)}
                }
            }
        }
    }
}
