//! The Source line room made editable: the lines of the text, each a node the surface can address,
//! with our own caret, selection and preedit drawn over them.

use super::act::Act;
use super::place::{caret_position, node_of, position_of, range_in, shown};
use crate::families::Held;
use crate::families::text::doc::TextDoc;
use crate::families::text::find::{FoundHits, Mark, pieces};
use crate::families::text::view::{hits_on, piece_class};
use crate::{Changes, HitIndex, Wrap};
use anyview_core::LineIndex;
use anyview_text::{Revision, Session, TokenClass, TokenLine, TokenSpan};
use dioxus::prelude::*;
use ds::edit::caret::use_caret_rect;
use ds::edit::handle::EditHandle;
use ds::edit::pointer::EditFocus;
use ds::edit::selection::use_selection_rects;
use ds::focus::soon::focus_soon;
use ds::host::position::{TextPosition, TextRange};
use ds::prelude::{EditSurface, Point, Px, Rect, Size, use_keys};
use ds::root::common::Common;
use ds::root::pass_through::ExtraClass;
use ds_core::word::Word;
use std::cell::RefCell;
use std::rc::Rc;

/// The most text before the room that is highlighted along with it: a file past this is shown
/// plain while it is edited, rather than parsed again after every key.
const HIGHLIGHT_BYTES: usize = 200_000;

/// The highlighted lines last made, and the text and window they were made for.
#[derive(Default)]
struct Cached {
    key: Option<(Revision, usize, usize)>,
    lines: Vec<TokenLine>,
}

/// An editor over the text of `session`, `rows` lines tall from line `first`. Scrolling to keep
/// the caret in view is asked of `onscroll`; a change of the text is told to `onchanged`.
#[component]
pub(crate) fn EditLines(
    session: Signal<Option<Session>>,
    handle: EditHandle,
    doc: Option<Held<TextDoc>>,
    first: u32,
    rows: u32,
    wrap: Wrap,
    hits: Option<Held<FoundHits>>,
    current: Option<HitIndex>,
    onscroll: EventHandler<u32>,
    onchanged: EventHandler<Changes>,
) -> Element {
    let goal_x = use_signal(|| None::<f32>);
    let upstream = use_signal(|| false);
    let mut focused = use_signal(|| false);
    let cache = use_hook(|| Rc::new(RefCell::new(Cached::default())));
    let act = Act {
        session,
        goal_x,
        upstream,
        handle,
        first,
        rows,
        scroll: onscroll,
        changed: onchanged,
        keys: use_keys(),
    };
    let frame = {
        let held = session.read();
        held.as_ref().map(|text| {
            Frame::of(
                text,
                Window {
                    first: first as usize,
                    rows: rows as usize,
                },
                Marks {
                    doc: doc.as_ref().map(|doc| doc.0.as_ref()),
                    hits: hits.as_ref(),
                    current,
                },
                &mut cache.borrow_mut(),
                upstream(),
            )
        })
    };
    let caret_rect = use_caret_rect(
        handle,
        frame
            .as_ref()
            .filter(|_| focused())
            .map(|frame| frame.caret.clone()),
    );
    let selection_rects = use_selection_rects(
        handle,
        frame.as_ref().and_then(|frame| frame.selection.clone()),
    );
    let tail_rects =
        use_selection_rects(handle, frame.as_ref().and_then(|frame| frame.tail.clone()));
    let code_left = use_caret_rect(handle, frame.as_ref().map(|frame| frame.left.clone()));
    let mut ime_area = use_signal(|| None::<Rect>);
    use_effect(move || {
        if let Some(area) = caret_rect().and_then(|rect| act.ime_area(rect)) {
            ime_area.set(Some(area));
        }
    });
    let Some(frame) = frame else {
        return rsx! {};
    };
    rsx! {
        EditSurface {
            common: Common {
                id: Some("editor".to_owned()),
                aria_label: Some("Text".to_owned()),
                extra_class: ExtraClass::parse("viewer-surface").ok(),
                mounted: Some(EventHandler::new(move |event: MountedEvent| focus_soon(event.data()))),
                ..Common::default()
            },
            handle,
            ime_area: ime_area(),
            on_input: move |input| act.input(input),
            on_pointer: move |pointer| act.pointer(pointer),
            on_focus: move |focus| focused.set(focus == EditFocus::In),
            div { class: "viewer-room",
                for line in frame.lines {
                    div { key: "{line.number}", class: "viewer-line", "data-wrap": wrap.slug(),
                        span { class: "viewer-lineno", "{line.number + 1}" }
                        span { class: "viewer-code", "data-edit-node": node_of(line.number),
                            for run in line.runs {
                                span { class: run.class, "{run.text}" }
                            }
                        }
                    }
                }
                for rect in only_text(selection_rects(), code_left()) {
                    div { class: "viewer-selection", style: boxed(rect) }
                }
                if let Some(rect) = drawn_caret(caret_rect(), upstream(), tail_rects()) {
                    div { class: "viewer-caret", style: boxed(rect) }
                }
            }
        }
    }
}

/// The lines the room holds.
#[derive(Debug, Clone, Copy)]
struct Window {
    first: usize,
    rows: usize,
}

/// What marks the lines: the highlighter of the file, and the hits of a find.
struct Marks<'a> {
    doc: Option<&'a TextDoc>,
    hits: Option<&'a Held<FoundHits>>,
    current: Option<HitIndex>,
}

/// What the room draws now.
struct Frame {
    lines: Vec<DrawnLine>,
    caret: TextPosition,
    /// The selection, cut to the lines drawn.
    selection: Option<TextRange>,
    /// The last row of a line, when the caret sits at its end rather than at the start of the
    /// row below.
    tail: Option<TextRange>,
    /// The start of the first line drawn: where the text begins, right of the gutter.
    left: TextPosition,
}

impl Frame {
    fn of(
        text: &Session,
        window: Window,
        marks: Marks<'_>,
        cache: &mut Cached,
        at_row_end: bool,
    ) -> Frame {
        let buffer = text.buffer();
        let count = buffer.line_count();
        let from = window.first.min(count - 1);
        let to = (from + window.rows).min(count);
        let tokens = tokens_for(text, marks.doc, cache, from, to);
        let preedit = shown(text).zip(text.preedit());
        let lines = buffer
            .lines(from, to - from)
            .into_iter()
            .zip(from..)
            .enumerate()
            .map(|(row, (line_text, number))| {
                let token_line = tokens
                    .get(row)
                    .filter(|token| token.text() == line_text)
                    .cloned()
                    .unwrap_or_else(|| plain(number, &line_text));
                let preedit_here = preedit
                    .filter(|(at, _)| at.line == number)
                    .map(|(at, edit)| (at.col, edit.text.as_str()));
                let (found, within) = hits_on(
                    marks.hits,
                    LineIndex(u32::try_from(number).unwrap_or(u32::MAX)),
                    marks.current,
                );
                DrawnLine {
                    number,
                    runs: runs(&token_line, preedit_here, found, within),
                }
            })
            .collect();
        let window_bytes = buffer.line_start(from)..buffer.line_range(to - 1).end;
        let end = text.caret();
        Frame {
            lines,
            caret: caret_position(text),
            selection: range_in(text, window_bytes),
            tail: at_row_end.then(|| TextRange {
                anchor: position_of(text, text.grapheme_before(end)),
                focus: position_of(text, end),
            }),
            left: TextPosition::new(node_of(from), 0),
        }
    }
}

/// The highlighted lines `from..to` of the text, made again only when the text or the window
/// changed. Lines past what the highlighter reads (a long file) come back unstyled, by there
/// being no token line for them.
fn tokens_for(
    text: &Session,
    doc: Option<&TextDoc>,
    cache: &mut Cached,
    from: usize,
    to: usize,
) -> Vec<TokenLine> {
    let key = (text.revision(), from, to);
    if cache.key != Some(key) {
        let end = text.buffer().line_start(to);
        cache.lines = match doc {
            Some(doc) if end <= HIGHLIGHT_BYTES => doc
                .highlight(&text.buffer().slice(0..end))
                .into_iter()
                .skip(from)
                .take(to - from)
                .collect(),
            Some(_) | None => Vec::new(),
        };
        cache.key = Some(key);
    }
    cache.lines.clone()
}

/// `text` as a line of one plain span.
fn plain(number: usize, text: &str) -> TokenLine {
    TokenLine {
        number: LineIndex(u32::try_from(number).unwrap_or(u32::MAX)),
        spans: if text.is_empty() {
            Vec::new()
        } else {
            vec![TokenSpan {
                class: TokenClass::Plain,
                text: text.to_owned(),
            }]
        },
    }
}

/// One line as drawn: its number and the runs of its text, the preedit among them.
struct DrawnLine {
    number: usize,
    runs: Vec<Run>,
}

struct Run {
    text: String,
    class: String,
}

/// The runs of `line`: cut where its hits begin and end, or, while the IME composes on it, with
/// the preedit put in at `preedit`'s column and no hits marked.
fn runs(
    line: &TokenLine,
    preedit: Option<(usize, &str)>,
    found: &[anyview_text::FindHit],
    within: Option<usize>,
) -> Vec<Run> {
    let Some((col, composing)) = preedit else {
        return pieces(line, found, within)
            .into_iter()
            .map(|piece| Run {
                class: piece_class(piece.class, piece.mark),
                text: piece.text,
            })
            .collect();
    };
    let (before, after) = split_at(line, col);
    let mut out: Vec<Run> = pieces(&before, &[], None)
        .into_iter()
        .map(|piece| Run {
            class: piece_class(piece.class, Mark::Plain),
            text: piece.text,
        })
        .collect();
    out.push(Run {
        class: "viewer-preedit".to_owned(),
        text: composing.to_owned(),
    });
    out.extend(pieces(&after, &[], None).into_iter().map(|piece| Run {
        class: piece_class(piece.class, Mark::Plain),
        text: piece.text,
    }));
    out
}

/// `line` cut at the byte column `col` into the line before it and the line after it.
fn split_at(line: &TokenLine, col: usize) -> (TokenLine, TokenLine) {
    let (mut before, mut after) = (Vec::new(), Vec::new());
    let mut start = 0;
    for span in &line.spans {
        let end = start + span.text.len();
        let cut = col.clamp(start, end) - start;
        let (head, tail) = span.text.split_at(if span.text.is_char_boundary(cut) {
            cut
        } else {
            0
        });
        if !head.is_empty() {
            before.push(TokenSpan {
                class: span.class,
                text: head.to_owned(),
            });
        }
        if !tail.is_empty() {
            after.push(TokenSpan {
                class: span.class,
                text: tail.to_owned(),
            });
        }
        start = end;
    }
    (
        TokenLine {
            number: line.number,
            spans: before,
        },
        TokenLine {
            number: line.number,
            spans: after,
        },
    )
}

/// The selection boxes over the lines' text: the surface covers every run of text between the two
/// ends of a selection, the line numbers' among them, so the boxes left of the text are dropped.
fn only_text(rects: Vec<Rect>, text: Option<Rect>) -> Vec<Rect> {
    let Some(text) = text else {
        return rects;
    };
    rects
        .into_iter()
        .filter(|rect| rect.origin.x.0 >= text.origin.x.0 - 0.5)
        .collect()
}

/// The caret's box: where the surface puts it, or, at the end of a wrapped row, just right of
/// the row's last character.
fn drawn_caret(at: Option<Rect>, upstream: bool, tail: Vec<Rect>) -> Option<Rect> {
    let caret = at?;
    match (upstream, tail.last()) {
        (true, Some(last)) => Some(Rect {
            origin: Point {
                x: Px(last.origin.x.0 + last.size.width.0),
                y: last.origin.y,
            },
            size: Size {
                width: caret.size.width,
                height: last.size.height,
            },
        }),
        (true, None) | (false, Some(_) | None) => Some(caret),
    }
}

/// A box placed in the room by its position and size, in the surface's logical pixels.
fn boxed(rect: Rect) -> String {
    format!(
        "left:{}px;top:{}px;width:{}px;height:{}px",
        rect.origin.x.0, rect.origin.y.0, rect.size.width.0, rect.size.height.0
    )
}
