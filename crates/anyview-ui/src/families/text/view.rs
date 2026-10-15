//! Text on screen: a clipped room of highlighted lines the worker read, or the Markdown page in
//! a sealed frame. The room owns the wheel (a window of lines scrolls by the line index, not by a
//! native scroll), so the machine's `Scroll(line)` is the one place the reader's position lives.

use super::doc::TextDoc;
use super::edit::lines::EditLines;
use super::find::{FoundHits, Mark, pieces};
use super::frame;
use super::tabs;
use crate::families::view::{Held, StageCx};
use crate::views::failed::{FailedScreen, Offer};
use crate::{
    Changes, ContentScale, FindHits, LoadFailure, Stage, StageIn, TextIn, TextPlace, TextStage,
    TextView as Shown, Wrap,
};
use anyview_core::LineIndex;
use anyview_text::{FindHit, TOKEN_CLASS_PREFIX, TokenClass, TokenLine, mark_page};
use dioxus::prelude::*;
use ds::host::gesture::{Gesture, WheelDelivery, use_gestures_with};
use ds::prelude::Word;
use ds_blitz::{FrameTag, use_scroll_handle};

/// The height of one line in logical pixels: the `--s-20` step of the stylesheet.
const ROW: f32 = 20.0;

/// The height of the titlebar in logical pixels: the `--chrome-titlebar-height` token, which the
/// stage's top starts below (`.viewer-scrolls`, views/window.css). A pane has no titlebar, so
/// counting it there only makes a page a line or so short.
const TITLEBAR: f32 = 28.0;

/// The tag of the page's frame, by which the window finds it to scroll it.
const FRAME_TAG: &str = "page";

/// Lines drawn past the ones that fit, so a partial row at the bottom is never blank.
const SPARE: u32 = 2;

/// Lines asked for beyond the room, so a short scroll lands on lines already read.
const OVERSCAN: u32 = 8;

pub(super) fn place_of(stage: &Stage) -> Option<TextPlace> {
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
        TextStage::Reading { place }
        | TextStage::Finding { place, .. }
        | TextStage::Editing { place, .. } => *place,
    }
}

/// The find the stage has up: its text and where its search stands.
fn find_of(stage: &Stage) -> Option<(crate::TypedText, FindHits)> {
    match stage {
        Stage::Text(
            TextStage::Finding { query, hits, .. }
            | TextStage::Editing {
                find: Some(crate::StageFind { query, hits }),
                ..
            },
        ) => Some((query.clone(), *hits)),
        Stage::Text(TextStage::Reading { .. } | TextStage::Editing { find: None, .. })
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}

/// How many lines fit the room that the stage's `height` leaves below the titlebar, at `scale`.
pub(super) fn rows_of(height: f32, scale: ContentScale) -> u32 {
    // A positive, finite height over a positive length: truncating to whole rows is the point.
    ((height - TITLEBAR) / (ROW * scale.factor()))
        .floor()
        .max(1.0) as u32
}

fn class_of(class: TokenClass) -> String {
    format!("{TOKEN_CLASS_PREFIX}{}", class.slug())
}

/// The class a piece of a line wears: its token class, and a mark when it is part of a hit.
pub(super) fn piece_class(class: TokenClass, mark: Mark) -> String {
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
    // A tab is drawn as spaces to the next stop, the line's pieces counting on from each other.
    let mut column = 0;
    let drawn: Vec<String> = cut
        .iter()
        .map(|piece| tabs::expand(&piece.text, &mut column))
        .collect();
    rsx! {
        div { class: "viewer-line", "data-wrap": wrap.slug(),
            span { class: "viewer-lineno", "{number}" }
            span { class: "viewer-code",
                for (piece, text) in cut.iter().zip(drawn.iter()) {
                    span { class: piece_class(piece.class, piece.mark), "{text}" }
                }
            }
        }
    }
}

/// The hits on `line` and the position among them of the current hit (`current`, among all).
pub(super) fn hits_on(
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
    // A wheel's detents arrive eased, one share a frame; `carry` keeps what is below a line.
    use_gestures_with(WheelDelivery::Eased, move |gesture| {
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
        let row = ROW * place.scale.factor();
        let total = carry() - by.y.0;
        let lines = (total / row).trunc();
        carry.set(total - lines * row);
        if lines != 0.0 {
            // The text being edited has the lines the edits left it.
            let total = scrolled.text_edit.peek().as_ref().map_or(count, |text| {
                u32::try_from(text.buffer().line_count()).unwrap_or(u32::MAX)
            });
            let last = total.saturating_sub(1);
            let line = (i64::from(place.line.0) + lines as i64).clamp(0, i64::from(last));
            send.call(StageIn::Text(TextIn::Scroll(LineIndex(
                u32::try_from(line).unwrap_or(0),
            ))));
        }
    });

    let (Some(area), Some(place)) = (cx.area, place_of(&cx.stage)) else {
        return rsx! { div { class: "viewer-text viewer-scrolls" } };
    };
    let page = rows_of(area.size.height.0, place.scale);
    let rows = page + SPARE;
    let first = place.line;
    let zoom = format!("--zoom:{}", place.scale.factor());
    // Once the text is read for editing, the room shows it and not the lines a worker read.
    let editing =
        matches!(cx.stage, Stage::Text(TextStage::Editing { .. })) && cx.text_edit.read().is_some();
    let covered = editing
        || cx.lines.as_ref().is_some_and(|window| {
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

    // The rendered page is a sealed frame, which the room cannot scroll by line: when the current
    // hit changes, the window scrolls the frame to the mark of it once the frame has laid the
    // marked page out.
    let frame_scroll = use_scroll_handle();
    let paged = matches!(place.view, Shown::Rendered) && doc.0.rendered.is_some();
    let on_hit =
        find_of(&cx.stage).and_then(|(query, hits)| hits.current().map(|hit| (query, hit)));
    use_effect(use_reactive!(|on_hit, paged| {
        if let (Some(handle), Some((_, hit)), true) = (&frame_scroll, on_hit, paged) {
            // The mark's number names this hit, so the page still marked for the last one is
            // waited out rather than scrolled.
            let mark = format!("mark[data-current=\"{}\"]", hit.0);
            handle.frame_into_view(FrameTag::new(FRAME_TAG), mark);
        }
    }));

    // A file of no bytes shows what an empty picture or recording shows. Editing it is still
    // offered, and the editor over it is empty.
    if doc.0.is_empty() && !matches!(cx.stage, Stage::Text(TextStage::Editing { .. })) {
        return rsx! {
            FailedScreen {
                reason: LoadFailure::Empty,
                name: String::new(),
                offer: Offer::Nothing,
                onreveal: |()| {},
            }
        };
    }

    let find = find_of(&cx.stage);
    let current = find.as_ref().and_then(|(_, hits)| hits.current());
    if let (Shown::Rendered, Some(rendered)) = (place.view, doc.0.rendered.as_ref()) {
        // A find over the page marks its hits in the page, the current one apart.
        let needle = find
            .as_ref()
            .and(cx.hits.as_ref())
            .and_then(|found| found.0.page_needle());
        let page = match needle {
            Some(needle) => {
                // a u32 fits a usize
                let marked = mark_page(&rendered.html, needle, current.map(|hit| hit.0 as usize));
                frame::document(&marked, &cx.frame, place.scale)
            }
            None => frame::document(&rendered.html, &cx.frame, place.scale),
        };
        return rsx! {
            div { class: "viewer-text viewer-scrolls", "data-view": "rendered", style: "{zoom}",
                iframe { class: "viewer-frame", "data-frame-tag": FRAME_TAG, srcdoc: page }
            }
        };
    }
    if editing {
        return rsx! {
            div { class: "viewer-text viewer-scrolls", "data-view": "source", "data-editing": "on", style: "{zoom}",
                EditLines {
                    session: cx.text_edit,
                    handle: cx.text_editor,
                    doc: Some(doc.clone()),
                    first: first.0,
                    rows,
                    page,
                    wrap: place.wrap,
                    hits: cx.hits.clone(),
                    current,
                    onscroll: move |line: u32| send.call(StageIn::Text(TextIn::Scroll(LineIndex(line)))),
                    onchanged: move |changes: Changes| send.call(StageIn::Text(TextIn::Edited(changes))),
                }
            }
        };
    }
    rsx! {
        div { class: "viewer-text viewer-scrolls", "data-view": "source", style: "{zoom}",
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_that_fit_are_counted_below_the_titlebar_and_are_at_least_one() {
        // name, the stage's height, rows
        const CASES: &[(&str, f32, u32)] = &[
            ("a few", 100.0, 3),
            ("exactly two", TITLEBAR + 2.0 * ROW, 2),
            ("less than the bar", 10.0, 1),
        ];
        for (name, height, rows) in CASES {
            assert_eq!(rows_of(*height, ContentScale::ACTUAL), *rows, "{name}");
        }
    }
}
