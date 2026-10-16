//! A mark: its tool, its shape, and where it is on a page.

use super::MarkColor;
use super::page::{PagePoint, PageRect, shifted};
use crate::units::PageIndex;
use ds_core::word::Word;

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
    /// Draw freehand.
    Sketch,
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
    /// Freehand strokes, each the points the pointer went through.
    Ink(Vec<Vec<PagePoint>>),
    /// A box with words in it.
    TextBox {
        /// The box.
        rect: PageRect,
        /// What it says.
        text: String,
    },
}

/// The file's own name for an annotation it already held when it was opened: what a save needs to
/// find that annotation again and change or remove it. The numbers mean nothing to anyone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkRef {
    /// The number of the annotation's object in the file.
    pub number: u32,
    /// The generation of that object.
    pub generation: u16,
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
    /// The annotation the file already held that this mark stands for; `None` for a mark made in
    /// this session, which a save adds.
    pub existing: Option<MarkRef>,
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
            MarkShape::Ink(strokes) => strokes
                .iter()
                .flatten()
                .map(|point| PageRect::spanning(*point, *point))
                .reduce(PageRect::joined)
                .unwrap_or(PageRect::spanning(
                    PagePoint::default(),
                    PagePoint::default(),
                )),
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
            MarkShape::Ink(strokes) => strokes.iter().any(|stroke| {
                stroke
                    .windows(2)
                    .any(|pair| nearer_than(pair[0], pair[1], at, reach))
                    || stroke.len() == 1
                        && stroke
                            .iter()
                            .any(|point| nearer_than(*point, *point, at, reach))
            }),
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
            i32::try_from(held.left().0).unwrap_or(0) - i32::try_from(bounds.left().0).unwrap_or(0),
            i32::try_from(held.top().0).unwrap_or(0) - i32::try_from(bounds.top().0).unwrap_or(0),
        );
        let shift = |rect: &PageRect| {
            PageRect::new(
                shifted(rect.left(), by.0),
                shifted(rect.top(), by.1),
                shifted(rect.right(), by.0),
                shifted(rect.bottom(), by.1),
            )
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
            MarkShape::Ink(strokes) => MarkShape::Ink(
                strokes
                    .iter()
                    .map(|stroke| stroke.iter().map(|point| point.moved(by.0, by.1)).collect())
                    .collect(),
            ),
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
    let (ax, ay) = (i64::from(a.x().0), i64::from(a.y().0));
    let (dx, dy) = (i64::from(b.x().0) - ax, i64::from(b.y().0) - ay);
    let (px, py) = (i64::from(at.x().0) - ax, i64::from(at.y().0) - ay);
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
