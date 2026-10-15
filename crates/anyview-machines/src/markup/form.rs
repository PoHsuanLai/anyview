//! Filling in the form: moving the keyboard between fields and writing their values.

use super::model::{MarkupOut, PdfMarkup, Typing};
use anyview_core::{Permille, WidgetKind};

/// How far above the top of a field the page is shown when the keyboard moves to it.
const ABOVE: u32 = 100;

impl PdfMarkup {
    // ----- the form -----

    /// The widgets the keyboard may go to, in the document's order.
    fn tab_order(&self) -> Vec<usize> {
        (0..self.widgets.len())
            .filter(|index| self.takes_input(*index))
            .collect()
    }

    /// Tab (`forward`) or ⇧Tab: the keyboard goes to the next field that takes a value, or the
    /// one before, wrapping round the document; the page is shown when the field is on another.
    pub(super) fn tab(&mut self, forward: bool, outs: &mut Vec<MarkupOut>) {
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
    pub(super) fn type_into(&mut self, index: usize, text: String) {
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
    pub(super) fn toggle(&mut self, index: usize) {
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
}
