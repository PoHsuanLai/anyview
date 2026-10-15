//! Making, picking, moving, colouring, writing in and deleting marks.

use super::model::{Drag, PdfMarkup, Typing, Using};
use anyview_core::{
    Mark, MarkShape, MarkTool, NOTE_SIZE, PageIndex, PagePoint, PageRect, Permille,
};

/// The least a rectangle, an oval or a line must be dragged to be drawn, in thousandths of the page.
const LEAST: u32 = 10;

/// The size of a text box made by a press with no drag, in thousandths of the page.
const BOX: (u32, u32) = (250, 40);

/// The default text box with its top left at `from`, held on the page.
fn box_at(from: PagePoint) -> PageRect {
    let left = from.x.0.min(1000 - BOX.0);
    let top = from.y.0.min(1000 - BOX.1);
    PageRect {
        left: Permille(left),
        top: Permille(top),
        right: Permille(left + BOX.0),
        bottom: Permille(top + BOX.1),
    }
}

impl PdfMarkup {
    // ----- the marks -----

    /// Highlight, underline or strike through the words that are selected.
    pub(super) fn apply(&mut self, tool: MarkTool) {
        if !self.can_mark() {
            return;
        }
        let Some(selected) = self.selected.take() else {
            return;
        };
        let shape = match tool {
            MarkTool::Highlight => MarkShape::Highlight(selected.lines),
            MarkTool::Underline => MarkShape::Underline(selected.lines),
            MarkTool::Strikethrough => MarkShape::Strikethrough(selected.lines),
            MarkTool::Pointer
            | MarkTool::Note
            | MarkTool::Rectangle
            | MarkTool::Oval
            | MarkTool::Line
            | MarkTool::Arrow
            | MarkTool::Sketch
            | MarkTool::TextBox => {
                self.selected = Some(selected);
                return;
            }
        };
        self.add(Mark {
            page: selected.page,
            color: self.color,
            shape,
            existing: None,
        });
    }

    /// The mark at `index` takes the colour chosen.
    pub(super) fn recolor(&mut self, index: usize) {
        let mut next = self.now.clone();
        if let Some(mark) = next.marks.get_mut(index) {
            mark.color = self.color;
            self.changed(next);
        }
    }

    pub(super) fn press(&mut self, page: PageIndex, at: PagePoint, reach: u32) {
        if self.using != Using::Marking || !self.can_mark() {
            return;
        }
        self.finish_writing();
        match self.tool {
            // The view selects the words; it hands them over with `Select` and `Apply`.
            MarkTool::Highlight | MarkTool::Underline | MarkTool::Strikethrough => {}
            MarkTool::Pointer => {
                let hit = self
                    .now
                    .marks
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, mark)| mark.page == page && mark.is_at(at, reach))
                    .map(|(index, _)| index);
                self.picked = hit;
                self.drag = hit.map(|index| Drag::Move {
                    index,
                    from: at,
                    base: self.now.clone(),
                });
            }
            MarkTool::Note => {
                let at = PagePoint::at(
                    at.x.0.min(1000 - NOTE_SIZE.0),
                    at.y.0.min(1000 - NOTE_SIZE.1),
                );
                let index = self.add(Mark {
                    page,
                    color: self.color,
                    shape: MarkShape::Note {
                        at,
                        text: String::new(),
                    },
                    existing: None,
                });
                self.picked = Some(index);
                self.writing = Some(index);
            }
            tool @ (MarkTool::Rectangle
            | MarkTool::Oval
            | MarkTool::Line
            | MarkTool::Arrow
            | MarkTool::Sketch
            | MarkTool::TextBox) => {
                self.picked = None;
                self.drag = Some(Drag::Draw {
                    page,
                    tool,
                    from: at,
                    to: at,
                    trail: if tool == MarkTool::Sketch {
                        vec![at]
                    } else {
                        Vec::new()
                    },
                });
            }
        }
    }

    pub(super) fn drag_to(&mut self, at: PagePoint) {
        match &mut self.drag {
            Some(Drag::Draw {
                to, trail, tool, ..
            }) => {
                *to = at;
                if *tool == MarkTool::Sketch && trail.last() != Some(&at) {
                    trail.push(at);
                }
            }
            Some(Drag::Move { index, from, base }) => {
                let by = (
                    i32::try_from(at.x.0).unwrap_or(0) - i32::try_from(from.x.0).unwrap_or(0),
                    i32::try_from(at.y.0).unwrap_or(0) - i32::try_from(from.y.0).unwrap_or(0),
                );
                let index = *index;
                if let Some(original) = base.marks.get(index) {
                    let moved = original.moved(by.0, by.1);
                    if let Some(mark) = self.now.marks.get_mut(index) {
                        *mark = moved;
                    }
                }
            }
            None => {}
        }
    }

    pub(super) fn release(&mut self) {
        match self.drag.take() {
            Some(Drag::Move { base, .. }) => {
                if self.now != base {
                    self.undo.push(base);
                    self.redo.clear();
                    self.typing = Typing::Idle;
                }
            }
            Some(Drag::Draw {
                page,
                tool,
                from,
                to,
                trail,
            }) => self.draw(page, tool, from, to, trail),
            None => {}
        }
    }

    /// The shape dragged from `from` to `to` becomes a mark, unless it is too small to be one.
    fn draw(
        &mut self,
        page: PageIndex,
        tool: MarkTool,
        from: PagePoint,
        to: PagePoint,
        trail: Vec<PagePoint>,
    ) {
        let rect = PageRect::spanning(from, to);
        let big = rect.width() >= LEAST && rect.height() >= LEAST;
        let long = rect.width() >= LEAST || rect.height() >= LEAST;
        let shape = match tool {
            MarkTool::Rectangle if big => MarkShape::Rectangle(rect),
            MarkTool::Oval if big => MarkShape::Oval(rect),
            MarkTool::Line if long => MarkShape::Line { from, to },
            MarkTool::Arrow if long => MarkShape::Arrow { from, to },
            MarkTool::Sketch if trail.len() >= 2 => MarkShape::Ink(vec![trail]),
            MarkTool::TextBox => MarkShape::TextBox {
                rect: if big { rect } else { box_at(from) },
                text: String::new(),
            },
            MarkTool::Rectangle
            | MarkTool::Oval
            | MarkTool::Line
            | MarkTool::Arrow
            | MarkTool::Sketch
            | MarkTool::Pointer
            | MarkTool::Highlight
            | MarkTool::Underline
            | MarkTool::Strikethrough
            | MarkTool::Note => return,
        };
        let writes = matches!(shape, MarkShape::TextBox { .. });
        let index = self.add(Mark {
            page,
            color: self.color,
            shape,
            existing: None,
        });
        self.picked = Some(index);
        if writes {
            self.writing = Some(index);
        }
    }

    /// `text` is all the words of the note or text box being written in.
    pub(super) fn write(&mut self, text: String) {
        let Some(index) = self.writing else {
            return;
        };
        if let Some(mark) = self.now.marks.get_mut(index) {
            match &mut mark.shape {
                MarkShape::Note { text: words, .. } | MarkShape::TextBox { text: words, .. } => {
                    *words = text;
                }
                MarkShape::Highlight(_)
                | MarkShape::Underline(_)
                | MarkShape::Strikethrough(_)
                | MarkShape::Rectangle(_)
                | MarkShape::Oval(_)
                | MarkShape::Line { .. }
                | MarkShape::Arrow { .. }
                | MarkShape::Ink(_) => {}
            }
        }
    }

    /// Writing ended. A note or text box that was just made and left with no words is taken back
    /// as if it had not been made.
    pub(super) fn finish_writing(&mut self) {
        let Some(index) = self.writing.take() else {
            return;
        };
        let empty = self
            .words_of(index)
            .is_some_and(|words| words.trim().is_empty());
        let just_made = self
            .undo
            .last()
            .is_some_and(|before| before.marks.len() + 1 == self.now.marks.len())
            && index + 1 == self.now.marks.len();
        if empty && just_made {
            if let Some(before) = self.undo.pop() {
                self.now = before;
            }
            self.picked = None;
        }
    }

    pub(super) fn delete(&mut self) {
        let Some(index) = self.picked else {
            return;
        };
        self.writing = None;
        self.picked = None;
        let mut next = self.now.clone();
        if index < next.marks.len() {
            next.marks.remove(index);
            self.changed(next);
        }
    }
}
