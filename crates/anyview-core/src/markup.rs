//! Markup: what a person writes on the pages of a document and the file keeps as standard
//! annotations: a highlight, an underline, a strike-through, a note, a rectangle, an oval, a line
//! or an arrow, a text box. Everything is placed as thousandths of the page as displayed (from the
//! top left), so a mark stays where it was put at any zoom and on a turned page.

use crate::units::{PageIndex, Permille};
use ds_core::word::Word;

/// A point on a page as displayed: each coordinate a thousandth of the page's width (`x`) or
/// height (`y`), from the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PagePoint {
    /// Across, 0 at the left edge and 1000 at the right.
    pub x: Permille,
    /// Down, 0 at the top edge and 1000 at the bottom.
    pub y: Permille,
}

/// `value` moved by `by`, held on the page.
fn shifted(value: Permille, by: i32) -> Permille {
    let moved = i64::from(value.0) + i64::from(by);
    Permille(u32::try_from(moved.clamp(0, 1000)).unwrap_or(0))
}

impl PagePoint {
    /// The point `x` and `y` thousandths in, held on the page.
    #[must_use]
    pub fn at(x: u32, y: u32) -> PagePoint {
        PagePoint {
            x: Permille(x.min(1000)),
            y: Permille(y.min(1000)),
        }
    }

    /// This point moved by `dx` across and `dy` down, held on the page.
    #[must_use]
    pub fn moved(self, dx: i32, dy: i32) -> PagePoint {
        PagePoint {
            x: shifted(self.x, dx),
            y: shifted(self.y, dy),
        }
    }
}

/// A rectangle on a page as displayed: each edge a thousandth of the page's width (left, right) or
/// height (top, bottom), from the top left, so 0 to 1000 spans the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageRect {
    /// The left edge.
    pub left: Permille,
    /// The top edge.
    pub top: Permille,
    /// The right edge.
    pub right: Permille,
    /// The bottom edge.
    pub bottom: Permille,
}

impl PageRect {
    /// The rectangle with these two opposite corners, whichever way round they are given.
    #[must_use]
    pub fn spanning(a: PagePoint, b: PagePoint) -> PageRect {
        PageRect {
            left: Permille(a.x.0.min(b.x.0)),
            top: Permille(a.y.0.min(b.y.0)),
            right: Permille(a.x.0.max(b.x.0)),
            bottom: Permille(a.y.0.max(b.y.0)),
        }
    }

    /// How wide it is, in thousandths of the page.
    #[must_use]
    pub fn width(self) -> u32 {
        self.right.0.saturating_sub(self.left.0)
    }

    /// How high it is, in thousandths of the page.
    #[must_use]
    pub fn height(self) -> u32 {
        self.bottom.0.saturating_sub(self.top.0)
    }

    /// Whether `at` is inside it or within `reach` thousandths of it.
    #[must_use]
    pub fn near(self, at: PagePoint, reach: u32) -> bool {
        at.x.0 + reach >= self.left.0
            && at.x.0 <= self.right.0 + reach
            && at.y.0 + reach >= self.top.0
            && at.y.0 <= self.bottom.0 + reach
    }

    /// Whether the two overlap.
    #[must_use]
    pub fn meets(self, other: PageRect) -> bool {
        self.left.0 < other.right.0
            && other.left.0 < self.right.0
            && self.top.0 < other.bottom.0
            && other.top.0 < self.bottom.0
    }

    /// The smallest rectangle holding both.
    #[must_use]
    pub fn joined(self, other: PageRect) -> PageRect {
        PageRect {
            left: Permille(self.left.0.min(other.left.0)),
            top: Permille(self.top.0.min(other.top.0)),
            right: Permille(self.right.0.max(other.right.0)),
            bottom: Permille(self.bottom.0.max(other.bottom.0)),
        }
    }

    /// This rectangle moved by `dx` across and `dy` down, as far as the page lets it go with its
    /// size kept.
    #[must_use]
    pub fn moved(self, dx: i32, dy: i32) -> PageRect {
        let room = |edge: u32, far: u32, by: i32| {
            let low = -i32::try_from(edge).unwrap_or(0);
            let high = i32::try_from(1000u32.saturating_sub(far)).unwrap_or(0);
            by.clamp(low, high)
        };
        let (dx, dy) = (
            room(self.left.0, self.right.0, dx),
            room(self.top.0, self.bottom.0, dy),
        );
        PageRect {
            left: shifted(self.left, dx),
            top: shifted(self.top, dy),
            right: shifted(self.right, dx),
            bottom: shifted(self.bottom, dy),
        }
    }
}

/// The colours a mark can have: the palette Preview offers, yellow first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum MarkColor {
    /// The highlighter's own colour.
    #[default]
    Yellow,
    /// Orange.
    Orange,
    /// Red.
    Red,
    /// Pink.
    Pink,
    /// Purple.
    Purple,
    /// Blue.
    Blue,
    /// Green.
    Green,
    /// Black.
    Black,
}

impl MarkColor {
    /// The colour as red, green and blue.
    #[must_use]
    pub const fn rgb(self) -> (u8, u8, u8) {
        match self {
            MarkColor::Yellow => (255, 214, 10),
            MarkColor::Orange => (255, 159, 10),
            MarkColor::Red => (255, 69, 58),
            MarkColor::Pink => (255, 105, 180),
            MarkColor::Purple => (191, 90, 242),
            MarkColor::Blue => (10, 132, 255),
            MarkColor::Green => (48, 209, 88),
            MarkColor::Black => (0, 0, 0),
        }
    }
}

/// What the Markup tool draws or does when the pointer is used on a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum MarkTool {
    /// Pick a mark to move it or delete it.
    #[default]
    Pointer,
    /// Highlight the words dragged over.
    Highlight,
    /// Underline the words dragged over.
    Underline,
    /// Strike through the words dragged over.
    #[word(label = "Strikethrough")]
    Strikethrough,
    /// Leave a note where the page is clicked.
    Note,
    /// Draw a rectangle.
    Rectangle,
    /// Draw an oval.
    Oval,
    /// Draw a line.
    Line,
    /// Draw an arrow.
    Arrow,
    /// Draw a box to type in.
    #[word(label = "Text Box")]
    TextBox,
}

impl MarkTool {
    /// Whether the tool marks words (a drag over text) rather than drawing.
    #[must_use]
    pub const fn marks_words(self) -> bool {
        matches!(
            self,
            MarkTool::Highlight | MarkTool::Underline | MarkTool::Strikethrough
        )
    }
}

/// The size of a note's icon on the page, in thousandths of the page's width and height.
pub const NOTE_SIZE: (u32, u32) = (32, 25);

/// What a mark is, and where.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MarkShape {
    /// The lines of text it covers.
    Highlight(Vec<PageRect>),
    /// The lines of text it runs under.
    Underline(Vec<PageRect>),
    /// The lines of text it crosses out.
    Strikethrough(Vec<PageRect>),
    /// A note whose icon has its top left corner here, and the words in it.
    Note {
        /// Where the icon sits.
        at: PagePoint,
        /// What the note says.
        text: String,
    },
    /// A rectangle.
    Rectangle(PageRect),
    /// An oval in a rectangle.
    Oval(PageRect),
    /// A line.
    Line {
        /// Where it starts.
        from: PagePoint,
        /// Where it ends.
        to: PagePoint,
    },
    /// A line with an arrow head at its end.
    Arrow {
        /// Where it starts.
        from: PagePoint,
        /// Where the head is.
        to: PagePoint,
    },
    /// A box with words in it.
    TextBox {
        /// The box.
        rect: PageRect,
        /// What it says.
        text: String,
    },
}

/// One mark on one page.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mark {
    /// The page it is on.
    pub page: PageIndex,
    /// Its colour.
    pub color: MarkColor,
    /// What it is.
    pub shape: MarkShape,
}

impl Mark {
    /// The smallest rectangle holding the whole mark.
    #[must_use]
    pub fn bounds(&self) -> PageRect {
        match &self.shape {
            MarkShape::Highlight(rects)
            | MarkShape::Underline(rects)
            | MarkShape::Strikethrough(rects) => rects
                .iter()
                .copied()
                .reduce(PageRect::joined)
                .unwrap_or(PageRect::spanning(
                    PagePoint::default(),
                    PagePoint::default(),
                )),
            MarkShape::Note { at, .. } => PageRect::spanning(
                *at,
                at.moved(
                    i32::try_from(NOTE_SIZE.0).unwrap_or(0),
                    i32::try_from(NOTE_SIZE.1).unwrap_or(0),
                ),
            ),
            MarkShape::Rectangle(rect)
            | MarkShape::Oval(rect)
            | MarkShape::TextBox { rect, .. } => *rect,
            MarkShape::Line { from, to } | MarkShape::Arrow { from, to } => {
                PageRect::spanning(*from, *to)
            }
        }
    }

    /// Whether `at` is on the mark, or within `reach` thousandths of it.
    #[must_use]
    pub fn is_at(&self, at: PagePoint, reach: u32) -> bool {
        match &self.shape {
            MarkShape::Highlight(rects)
            | MarkShape::Underline(rects)
            | MarkShape::Strikethrough(rects) => rects.iter().any(|rect| rect.near(at, reach)),
            MarkShape::Line { from, to } | MarkShape::Arrow { from, to } => {
                nearer_than(*from, *to, at, reach)
            }
            MarkShape::Note { .. }
            | MarkShape::Rectangle(_)
            | MarkShape::Oval(_)
            | MarkShape::TextBox { .. } => self.bounds().near(at, reach),
        }
    }

    /// This mark moved by `dx` across and `dy` down, as far as the page lets it go with its shape
    /// kept.
    #[must_use]
    pub fn moved(&self, dx: i32, dy: i32) -> Mark {
        let held = self.bounds().moved(dx, dy);
        let bounds = self.bounds();
        let by = (
            i32::try_from(held.left.0).unwrap_or(0) - i32::try_from(bounds.left.0).unwrap_or(0),
            i32::try_from(held.top.0).unwrap_or(0) - i32::try_from(bounds.top.0).unwrap_or(0),
        );
        let shift = |rect: &PageRect| PageRect {
            left: shifted(rect.left, by.0),
            top: shifted(rect.top, by.1),
            right: shifted(rect.right, by.0),
            bottom: shifted(rect.bottom, by.1),
        };
        let shape = match &self.shape {
            MarkShape::Highlight(rects) => MarkShape::Highlight(rects.iter().map(shift).collect()),
            MarkShape::Underline(rects) => MarkShape::Underline(rects.iter().map(shift).collect()),
            MarkShape::Strikethrough(rects) => {
                MarkShape::Strikethrough(rects.iter().map(shift).collect())
            }
            MarkShape::Note { at, text } => MarkShape::Note {
                at: at.moved(by.0, by.1),
                text: text.clone(),
            },
            MarkShape::Rectangle(rect) => MarkShape::Rectangle(shift(rect)),
            MarkShape::Oval(rect) => MarkShape::Oval(shift(rect)),
            MarkShape::Line { from, to } => MarkShape::Line {
                from: from.moved(by.0, by.1),
                to: to.moved(by.0, by.1),
            },
            MarkShape::Arrow { from, to } => MarkShape::Arrow {
                from: from.moved(by.0, by.1),
                to: to.moved(by.0, by.1),
            },
            MarkShape::TextBox { rect, text } => MarkShape::TextBox {
                rect: shift(rect),
                text: text.clone(),
            },
        };
        Mark {
            shape,
            ..self.clone()
        }
    }
}

/// Whether `at` is within `reach` of the segment from `a` to `b`.
fn nearer_than(a: PagePoint, b: PagePoint, at: PagePoint, reach: u32) -> bool {
    let (ax, ay) = (i64::from(a.x.0), i64::from(a.y.0));
    let (dx, dy) = (i64::from(b.x.0) - ax, i64::from(b.y.0) - ay);
    let (px, py) = (i64::from(at.x.0) - ax, i64::from(at.y.0) - ay);
    let length = dx * dx + dy * dy;
    // The foot of the perpendicular, as a share of the segment held between its ends.
    let along = if length == 0 {
        0
    } else {
        (px * dx + py * dy).clamp(0, length)
    };
    let (fx, fy) = if length == 0 {
        (0, 0)
    } else {
        (dx * along / length, dy * along / length)
    };
    let (ex, ey) = (px - fx, py - fy);
    ex * ex + ey * ey <= i64::from(reach) * i64::from(reach)
}

/// A value written into a field of the form: the field's name, and the text, or the state name of
/// a check box or radio button (`Off` when clear).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldFill {
    /// The field's fully qualified name.
    pub name: String,
    /// What it now holds.
    pub value: String,
}

/// Everything a person wrote on a document and has not saved: the fields they filled and the
/// marks they made.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MarkupSave {
    /// The fields filled, one entry for each.
    pub fills: Vec<FieldFill>,
    /// The marks made.
    pub marks: Vec<Mark>,
}

impl MarkupSave {
    /// Whether there is nothing to write.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fills.is_empty() && self.marks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
        PageRect {
            left: Permille(left),
            top: Permille(top),
            right: Permille(right),
            bottom: Permille(bottom),
        }
    }

    fn mark(shape: MarkShape) -> Mark {
        Mark {
            page: PageIndex(0),
            color: MarkColor::Yellow,
            shape,
        }
    }

    #[test]
    fn a_rectangle_moves_as_far_as_the_page_lets_it_with_its_size_kept() {
        // name, the move, where the rectangle ends
        let start = rect(100, 100, 300, 150);
        let cases = [
            ("free", (50, -20), rect(150, 80, 350, 130)),
            ("held at the left edge", (-500, 0), rect(0, 100, 200, 150)),
            (
                "held at the right edge",
                (900, 0),
                rect(800, 100, 1000, 150),
            ),
            ("held at the top", (0, -500), rect(100, 0, 300, 50)),
            ("held at the foot", (0, 2000), rect(100, 950, 300, 1000)),
        ];
        for (name, (dx, dy), want) in cases {
            assert_eq!(start.moved(dx, dy), want, "{name}");
        }
    }

    #[test]
    fn two_corners_make_the_same_rectangle_whichever_way_round() {
        let (a, b) = (PagePoint::at(300, 40), PagePoint::at(100, 90));
        assert_eq!(PageRect::spanning(a, b), rect(100, 40, 300, 90));
        assert_eq!(PageRect::spanning(b, a), rect(100, 40, 300, 90));
    }

    #[test]
    fn a_pointer_is_on_a_mark_when_it_is_in_reach_of_its_ink() {
        let line = mark(MarkShape::Line {
            from: PagePoint::at(100, 100),
            to: PagePoint::at(500, 100),
        });
        let words = mark(MarkShape::Highlight(vec![
            rect(100, 100, 400, 120),
            rect(100, 130, 300, 150),
        ]));
        // name, the mark, the point, whether it is on it
        let cases = [
            ("on the line", &line, PagePoint::at(300, 100), true),
            ("near the line", &line, PagePoint::at(300, 105), true),
            ("far from the line", &line, PagePoint::at(300, 160), false),
            ("past the line's end", &line, PagePoint::at(560, 100), false),
            (
                "on a highlighted line",
                &words,
                PagePoint::at(200, 110),
                true,
            ),
            (
                "between two highlighted lines",
                &words,
                PagePoint::at(350, 126),
                true,
            ),
            ("off a highlight", &words, PagePoint::at(700, 300), false),
        ];
        for (name, mark, at, on) in cases {
            assert_eq!(mark.is_at(at, 8), on, "{name}");
        }
    }

    #[test]
    fn moving_a_mark_moves_all_of_it_by_the_same_amount() {
        let words = mark(MarkShape::Underline(vec![
            rect(100, 100, 400, 120),
            rect(100, 130, 300, 150),
        ]));
        let moved = words.moved(10, 20);
        assert_eq!(
            moved.shape,
            MarkShape::Underline(vec![rect(110, 120, 410, 140), rect(110, 150, 310, 170)])
        );
        let line = mark(MarkShape::Arrow {
            from: PagePoint::at(950, 500),
            to: PagePoint::at(990, 520),
        });
        // Held at the edge as a whole: the line keeps its length.
        assert_eq!(
            line.moved(100, 0).shape,
            MarkShape::Arrow {
                from: PagePoint::at(960, 500),
                to: PagePoint::at(1000, 520),
            }
        );
    }

    #[test]
    fn a_note_covers_its_icon() {
        let note = mark(MarkShape::Note {
            at: PagePoint::at(100, 200),
            text: "x".into(),
        });
        assert_eq!(
            note.bounds(),
            rect(100, 200, 100 + NOTE_SIZE.0, 200 + NOTE_SIZE.1)
        );
    }
}
