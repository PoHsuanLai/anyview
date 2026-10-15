//! The markup's transitions.

use super::model::{Allowance, MarkupIn, MarkupOut, PdfMarkup, Selected, Typing, Using, Written};
use anyview_core::Mark;
use std::collections::BTreeMap;

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
    pub(super) fn changed(&mut self, next: Written) {
        if next == self.now {
            return;
        }
        let before = std::mem::replace(&mut self.now, next);
        self.undo.push(before);
        self.redo.clear();
        self.typing = Typing::Idle;
    }

    /// `mark` made, and its place among the marks.
    pub(super) fn add(&mut self, mark: Mark) -> usize {
        let mut next = self.now.clone();
        next.marks.push(mark);
        let index = next.marks.len() - 1;
        self.changed(next);
        index
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
