//! The markup's transitions.

use super::model::{
    Allowance, Drag, MarkupIn, MarkupOut, PdfMarkup, Selected, Typing, Using, Written,
};
use anyview_core::{
    Mark, MarkShape, MarkTool, NOTE_SIZE, PageIndex, PagePoint, PageRect, Permille, WidgetKind,
};
use std::collections::BTreeMap;

/// The least a rectangle, an oval or a line must be dragged to be drawn, in thousandths of the page.
const LEAST: u32 = 10;

/// The size of a text box made by a press with no drag, in thousandths of the page.
const BOX: (u32, u32) = (250, 40);

/// How far above the top of a field the page is shown when the keyboard moves to it.
const ABOVE: u32 = 100;

impl PdfMarkup {
    /// The markup after `input`, and what it asks for.
    #[must_use]
    pub fn step(self, input: MarkupIn) -> (PdfMarkup, Vec<MarkupOut>) {
        let mut outs = Vec::new();
        let next = self.stepped(input, &mut outs);
        (next, outs)
    }

    fn stepped(mut self, input: MarkupIn, outs: &mut Vec<MarkupOut>) -> PdfMarkup {
        match input {
            MarkupIn::Landed {
                widgets,
                filling,
                marking,
                marks,
            } => {
                let opened = Written {
                    fills: BTreeMap::new(),
                    marks: marks.to_vec(),
                };
                self = PdfMarkup {
                    widgets,
                    filling,
                    marking,
                    now: opened.clone(),
                    opened,
                    using: self.using,
                    tool: self.tool,
                    color: self.color,
                    ..PdfMarkup::default()
                };
            }
            MarkupIn::Using(using) => self.use_pointer(using),
            MarkupIn::Focus(widget) => {
                if self.takes_input(widget) && self.focus != Some(widget) {
                    self.focus = Some(widget);
                    self.typing = Typing::Idle;
                }
            }
            MarkupIn::Blur(widget) => {
                if self.focus == Some(widget) {
                    self.focus = None;
                    self.typing = Typing::Idle;
                }
            }
            MarkupIn::Next => self.tab(true, outs),
            MarkupIn::Previous => self.tab(false, outs),
            MarkupIn::Type { widget, text } => self.type_into(widget, text),
            MarkupIn::Toggle(widget) => self.toggle(widget),
            MarkupIn::Tool(tool) => {
                if self.can_mark() {
                    self.finish_writing();
                    self.tool = tool;
                    self.picked = None;
                    self.drag = None;
                }
            }
            MarkupIn::Color(color) => {
                self.color = color;
                if let Some(index) = self.picked {
                    self.recolor(index);
                }
            }
            MarkupIn::Select(selection) => {
                self.selected = selection
                    .filter(|(_, lines)| !lines.is_empty())
                    .map(|(page, lines)| Selected { page, lines });
            }
            MarkupIn::Apply(tool) => self.apply(tool),
            MarkupIn::Press { page, at, reach } => self.press(page, at, reach),
            MarkupIn::Drag(at) => self.drag_to(at),
            MarkupIn::Release => self.release(),
            MarkupIn::Words(text) => self.write(text),
            MarkupIn::Write(index) => {
                if self.can_mark() && self.words_of(index).is_some() {
                    self.finish_writing();
                    self.writing = Some(index);
                    self.picked = Some(index);
                }
            }
            MarkupIn::Done => self.finish_writing(),
            MarkupIn::Delete => self.delete(),
            MarkupIn::Clear => {
                self.finish_writing();
                self.selected = None;
                self.picked = None;
                self.drag = None;
            }
            MarkupIn::Discard => {
                self.now = self.opened.clone();
                self.undo.clear();
                self.redo.clear();
                self.typing = Typing::Idle;
                self.focus = None;
                self.selected = None;
                self.drag = None;
                self.picked = None;
                self.writing = None;
            }
            MarkupIn::Undo => self.rewind(true),
            MarkupIn::Redo => self.rewind(false),
        }
        self
    }

    /// The pointer tool changed: what is selected or half done goes, and the Markup tool is only
    /// taken where marks can be made.
    fn use_pointer(&mut self, using: Using) {
        let using = match (using, self.marking) {
            (Using::Marking, Allowance::Withheld) => Using::Reading,
            (Using::Reading | Using::Selecting | Using::Marking, _) => using,
        };
        if using != self.using {
            self.finish_writing();
            self.selected = None;
            self.drag = None;
            self.picked = None;
            self.using = using;
        }
    }

    /// `next` is what has been written now, and what was written is kept to take back. Writing the
    /// same again changes nothing, and anything that had been taken back can no longer be done again.
    fn changed(&mut self, next: Written) {
        if next == self.now {
            return;
        }
        let before = std::mem::replace(&mut self.now, next);
        self.undo.push(before);
        self.redo.clear();
        self.typing = Typing::Idle;
    }

    /// `mark` made, and its place among the marks.
    fn add(&mut self, mark: Mark) -> usize {
        let mut next = self.now.clone();
        next.marks.push(mark);
        let index = next.marks.len() - 1;
        self.changed(next);
        index
    }

    // ----- the form -----

    /// The widgets the keyboard may go to, in the document's order.
    fn tab_order(&self) -> Vec<usize> {
        (0..self.widgets.len())
            .filter(|index| self.takes_input(*index))
            .collect()
    }

    /// Tab (`forward`) or ⇧Tab: the keyboard goes to the next field that takes a value, or the
    /// one before, wrapping round the document; the page is shown when the field is on another.
    fn tab(&mut self, forward: bool, outs: &mut Vec<MarkupOut>) {
        let order = self.tab_order();
        let (Some(first), Some(last)) = (order.first().copied(), order.last().copied()) else {
            return;
        };
        let here = self
            .focus
            .and_then(|focus| order.iter().position(|index| *index == focus));
        let target = match (here, forward) {
            (Some(at), true) => order.get(at + 1).copied().unwrap_or(first),
            (Some(at), false) => at
                .checked_sub(1)
                .and_then(|before| order.get(before).copied())
                .unwrap_or(last),
            (None, true) => first,
            (None, false) => last,
        };
        let from_page = self
            .focus
            .and_then(|focus| self.widgets.get(focus))
            .map(|widget| widget.page);
        if let Some(widget) = self.widgets.get(target)
            && from_page != Some(widget.page)
        {
            outs.push(MarkupOut::Reveal {
                page: widget.page,
                offset: Permille(widget.rect.top.0.saturating_sub(ABOVE)),
            });
        }
        self.focus = Some(target);
        self.typing = Typing::Idle;
    }

    /// The field of the widget at `index` now holds `text`. Letters typed one after another into
    /// a text field are one step to take back.
    fn type_into(&mut self, index: usize, text: String) {
        if !self.takes_input(index) {
            return;
        }
        let Some(widget) = self.widgets.get(index) else {
            return;
        };
        let typing = match widget.kind {
            WidgetKind::Text | WidgetKind::TextArea | WidgetKind::Secret => {
                Typing::Into(widget.name.clone())
            }
            WidgetKind::Choice | WidgetKind::List => Typing::Idle,
            WidgetKind::Check | WidgetKind::Radio => return,
        };
        let name = widget.name.clone();
        self.fill(&name, text, typing);
    }

    /// A press on a check box (on or off) or a radio button (on; a group always has one on once
    /// one has been chosen).
    fn toggle(&mut self, index: usize) {
        if !self.takes_input(index) {
            return;
        }
        let Some(widget) = self.widgets.get(index) else {
            return;
        };
        let on = self.is_on(index);
        let value = match (widget.kind, on) {
            (WidgetKind::Check, true) => "Off".to_owned(),
            (WidgetKind::Check | WidgetKind::Radio, false) => widget.on.clone(),
            (WidgetKind::Radio, true) => return,
            (
                WidgetKind::Text
                | WidgetKind::TextArea
                | WidgetKind::Secret
                | WidgetKind::Choice
                | WidgetKind::List,
                _,
            ) => return,
        };
        let name = widget.name.clone();
        self.fill(&name, value, Typing::Idle);
    }

    /// The field `name` holds `value`. A value the file already holds is no change at all.
    fn fill(&mut self, name: &str, value: String, typing: Typing) {
        let original = self
            .widgets
            .iter()
            .find(|widget| widget.name == name)
            .map_or("", |widget| widget.value.as_str());
        let mut next = self.now.clone();
        if value == original {
            next.fills.remove(name);
        } else {
            next.fills.insert(name.to_owned(), value);
        }
        if next == self.now {
            self.typing = typing;
            return;
        }
        let same_run = typing != Typing::Idle && typing == self.typing && !self.undo.is_empty();
        if same_run {
            self.now = next;
            self.redo.clear();
        } else {
            self.changed(next);
        }
        self.typing = typing;
    }

    // ----- the marks -----

    /// Highlight, underline or strike through the words that are selected.
    fn apply(&mut self, tool: MarkTool) {
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
    fn recolor(&mut self, index: usize) {
        let mut next = self.now.clone();
        if let Some(mark) = next.marks.get_mut(index) {
            mark.color = self.color;
            self.changed(next);
        }
    }

    fn press(&mut self, page: PageIndex, at: PagePoint, reach: u32) {
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

    fn drag_to(&mut self, at: PagePoint) {
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

    fn release(&mut self) {
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
    fn write(&mut self, text: String) {
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
    fn finish_writing(&mut self) {
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

    fn delete(&mut self) {
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

    /// Take back (`undoing`) or do again the last change.
    fn rewind(&mut self, undoing: bool) {
        let (from, to) = if undoing {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        if let Some(there) = from.pop() {
            to.push(std::mem::replace(&mut self.now, there));
        }
        self.picked = None;
        self.writing = None;
        self.drag = None;
        self.typing = Typing::Idle;
    }
}

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
