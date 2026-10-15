//! A mark drawn over its page: the boxes and rules that stand for it, each placed as a fraction of
//! the page's box so a mark stays where it was put at any zoom. A line is a rule turned about its
//! start, so its length and angle are worked out in the page's own pixels.

use super::scene::percent;
use anyview_core::{Mark, MarkColor, MarkShape, NOTE_SIZE, PagePoint, PageRect, Permille};
use ds_core::word::Word;

/// How long each stroke of an arrow's head is, in logical pixels.
const HEAD: f32 = 12.0;

/// How far each stroke of an arrow's head leans from the line, in radians.
const LEAN: f32 = 0.45;

/// What an ink looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(super) enum InkKind {
    /// A translucent fill: a highlight, the words selected.
    Fill,
    /// A rule across the width of a box: an underline, a strike-through.
    Rule,
    /// A rectangle's outline.
    Frame,
    /// An oval's outline.
    Round,
    /// A line, or one stroke of an arrow's head.
    Stroke,
    /// Freehand strokes, drawn as a polyline in the box that holds them.
    Sketch,
    /// A note's icon.
    Note,
    /// A text box with its words.
    Words,
}

/// Whether an ink is a mark at rest, the one picked, a shape being drawn, or the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(super) enum InkState {
    /// A mark.
    Resting,
    /// The mark that is picked.
    Picked,
    /// A shape the pointer is drawing.
    Draft,
    /// Words selected.
    Selected,
}

/// One box of a mark, drawn.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Ink {
    pub kind: InkKind,
    pub state: InkState,
    /// Where it is and what colour: inline CSS.
    pub style: String,
    /// The words of a text box.
    pub text: String,
    /// The points of a sketch's strokes, each stroke as `x,y` pairs in logical pixels from the top
    /// left of its box, the strokes divided by `|`.
    pub path: String,
    /// The width and height of the box a sketch is drawn in, in logical pixels, never less than
    /// one, so the strokes are drawn at the page's own scale.
    pub extent: (f32, f32),
}

fn color(color: MarkColor) -> String {
    let (red, green, blue) = color.rgb();
    format!("--mark:rgb({red}, {green}, {blue})")
}

fn boxed(color: &str, rect: PageRect) -> String {
    let (left, top, width, height) = percent(rect);
    let lead = if color.is_empty() {
        String::new()
    } else {
        format!("{color};")
    };
    format!("{lead}left:{left:.1}%;top:{top:.1}%;width:{width:.1}%;height:{height:.1}%")
}

fn ink(kind: InkKind, state: InkState, style: String) -> Ink {
    Ink {
        kind,
        state,
        style,
        text: String::new(),
        path: String::new(),
        extent: (1.0, 1.0),
    }
}

/// A stroke of `length` logical pixels from `from`, turned `angle` radians clockwise.
fn stroke_from(color: &str, from: PagePoint, length: f32, angle: f32, state: InkState) -> Ink {
    let (left, top) = (from.x.0 as f32 / 10.0, from.y.0 as f32 / 10.0);
    ink(
        InkKind::Stroke,
        state,
        format!(
            "{color};left:{left:.2}%;top:{top:.2}%;width:{length:.1}px;transform:rotate({angle:.4}rad)"
        ),
    )
}

/// A line from `from` to `to` on a page `size` logical pixels across and down, with an arrow head
/// at its end when `arrow`.
fn line(
    color: &str,
    (from, to): (PagePoint, PagePoint),
    size: (f32, f32),
    arrow: bool,
    state: InkState,
) -> Vec<Ink> {
    let across = (to.x.0 as f32 - from.x.0 as f32) / 1000.0 * size.0;
    let down = (to.y.0 as f32 - from.y.0 as f32) / 1000.0 * size.1;
    let angle = down.atan2(across);
    let mut strokes = vec![stroke_from(color, from, across.hypot(down), angle, state)];
    if arrow {
        for lean in [LEAN, -LEAN] {
            strokes.push(stroke_from(
                color,
                to,
                HEAD,
                angle + std::f32::consts::PI + lean,
                state,
            ));
        }
    }
    strokes
}

/// The points of `strokes` as `x,y` pairs in logical pixels from the top left of `rect`, on a page
/// `size` logical pixels across and down; a stroke's pairs separated by spaces and the strokes by
/// `|`.
fn stroke_points(strokes: &[Vec<PagePoint>], rect: PageRect, size: (f32, f32)) -> String {
    strokes
        .iter()
        .map(|stroke| {
            stroke
                .iter()
                .map(|point| {
                    let x = point.x.0.saturating_sub(rect.left.0) as f32 * size.0 / 1000.0;
                    let y = point.y.0.saturating_sub(rect.top.0) as f32 * size.1 / 1000.0;
                    format!("{x:.1},{y:.1}")
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// `mark` drawn on a page `size` logical pixels across and down.
pub(super) fn inks_of(mark: &Mark, size: (f32, f32), state: InkState) -> Vec<Ink> {
    let paint = color(mark.color);
    match &mark.shape {
        MarkShape::Highlight(rects) => rects
            .iter()
            .map(|rect| ink(InkKind::Fill, state, boxed(&paint, *rect)))
            .collect(),
        MarkShape::Underline(rects) => rects
            .iter()
            .map(|rect| {
                let at = PageRect {
                    top: rect.bottom,
                    ..*rect
                };
                ink(InkKind::Rule, state, boxed(&paint, at))
            })
            .collect(),
        MarkShape::Strikethrough(rects) => rects
            .iter()
            .map(|rect| {
                let middle = Permille(rect.top.0 + rect.height() / 2);
                let at = PageRect {
                    top: middle,
                    bottom: middle,
                    ..*rect
                };
                ink(InkKind::Rule, state, boxed(&paint, at))
            })
            .collect(),
        MarkShape::Note { at, .. } => {
            let icon = PageRect::spanning(
                *at,
                at.moved(
                    i32::try_from(NOTE_SIZE.0).unwrap_or(0),
                    i32::try_from(NOTE_SIZE.1).unwrap_or(0),
                ),
            );
            vec![ink(InkKind::Note, state, boxed(&paint, icon))]
        }
        MarkShape::Rectangle(rect) => vec![ink(InkKind::Frame, state, boxed(&paint, *rect))],
        // An oval is the mark's own shape, not a corner of the chrome: an ellipse of its box.
        MarkShape::Oval(rect) => vec![ink(
            InkKind::Round,
            state,
            format!("{};border-radius:50%", boxed(&paint, *rect)),
        )],
        MarkShape::Line { from, to } => line(&paint, (*from, *to), size, false, state),
        MarkShape::Arrow { from, to } => line(&paint, (*from, *to), size, true, state),
        MarkShape::Ink(strokes) => {
            let rect = mark.bounds();
            vec![Ink {
                path: stroke_points(strokes, rect, size),
                extent: (
                    (rect.width() as f32 * size.0 / 1000.0).max(1.0),
                    (rect.height() as f32 * size.1 / 1000.0).max(1.0),
                ),
                ..ink(InkKind::Sketch, state, boxed(&paint, rect))
            }]
        }
        MarkShape::TextBox { rect, text } => vec![Ink {
            text: text.clone(),
            ..ink(InkKind::Words, state, boxed(&paint, *rect))
        }],
    }
}

/// The words selected, as one fill for each line.
pub(super) fn selection(lines: &[PageRect]) -> Vec<Ink> {
    lines
        .iter()
        .map(|rect| ink(InkKind::Fill, InkState::Selected, boxed("", *rect)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::PageIndex;

    fn mark(shape: MarkShape) -> Mark {
        Mark {
            page: PageIndex(0),
            color: MarkColor::Blue,
            shape,
            existing: None,
        }
    }

    fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
        PageRect {
            left: Permille(left),
            top: Permille(top),
            right: Permille(right),
            bottom: Permille(bottom),
        }
    }

    #[test]
    fn each_shape_is_drawn_as_the_boxes_that_stand_for_it() {
        let size = (500.0, 1000.0);
        // name, the mark, the kinds of its inks
        let cases = [
            (
                "a highlight is a fill for each line",
                MarkShape::Highlight(vec![rect(0, 0, 100, 20), rect(0, 30, 80, 50)]),
                vec![InkKind::Fill, InkKind::Fill],
            ),
            (
                "an underline is a rule",
                MarkShape::Underline(vec![rect(0, 0, 100, 20)]),
                vec![InkKind::Rule],
            ),
            (
                "a rectangle",
                MarkShape::Rectangle(rect(0, 0, 100, 20)),
                vec![InkKind::Frame],
            ),
            (
                "an oval",
                MarkShape::Oval(rect(0, 0, 100, 20)),
                vec![InkKind::Round],
            ),
            (
                "a line is one stroke",
                MarkShape::Line {
                    from: PagePoint::at(0, 0),
                    to: PagePoint::at(100, 100),
                },
                vec![InkKind::Stroke],
            ),
            (
                "an arrow is its line and two strokes of the head",
                MarkShape::Arrow {
                    from: PagePoint::at(0, 0),
                    to: PagePoint::at(100, 100),
                },
                vec![InkKind::Stroke, InkKind::Stroke, InkKind::Stroke],
            ),
            (
                "a sketch is one box holding its strokes",
                MarkShape::Ink(vec![vec![PagePoint::at(0, 0), PagePoint::at(100, 100)]]),
                vec![InkKind::Sketch],
            ),
            (
                "a note is its icon",
                MarkShape::Note {
                    at: PagePoint::at(10, 10),
                    text: String::new(),
                },
                vec![InkKind::Note],
            ),
        ];
        for (name, shape, kinds) in cases {
            let got: Vec<InkKind> = inks_of(&mark(shape), size, InkState::Resting)
                .iter()
                .map(|ink| ink.kind)
                .collect();
            assert_eq!(got, kinds, "{name}");
        }
    }

    #[test]
    fn a_line_is_a_rule_as_long_and_as_turned_as_it_is_on_the_page() {
        // A page 500 across and 1000 down: from a tenth across and half way down, to the point
        // half a page further on the same height (250 pixels).
        let level = inks_of(
            &mark(MarkShape::Line {
                from: PagePoint::at(100, 500),
                to: PagePoint::at(600, 500),
            }),
            (500.0, 1000.0),
            InkState::Resting,
        );
        assert_eq!(level.len(), 1);
        assert!(level[0].style.contains("left:10.00%"), "{}", level[0].style);
        assert!(
            level[0].style.contains("width:250.0px"),
            "{}",
            level[0].style
        );
        assert!(
            level[0].style.contains("rotate(0.0000rad)"),
            "{}",
            level[0].style
        );
        // Straight down 100 thousandths of the page is 100 pixels, a quarter turn.
        let down = inks_of(
            &mark(MarkShape::Line {
                from: PagePoint::at(100, 100),
                to: PagePoint::at(100, 200),
            }),
            (500.0, 1000.0),
            InkState::Resting,
        );
        assert!(down[0].style.contains("width:100.0px"), "{}", down[0].style);
        assert!(
            down[0].style.contains("rotate(1.5708rad)"),
            "{}",
            down[0].style
        );
    }

    #[test]
    fn the_selection_is_a_fill_for_each_line_in_the_selected_state() {
        let inks = selection(&[rect(0, 0, 100, 20), rect(0, 30, 80, 50)]);
        assert_eq!(inks.len(), 2);
        assert!(
            inks.iter()
                .all(|ink| ink.kind == InkKind::Fill && ink.state == InkState::Selected)
        );
    }
}
