//! What the markup says about itself: what to save, what is picked, drawn and written.

use super::model::{Allowance, Draft, Drag, PdfMarkup, Using};
use anyview_core::{
    FormWidget, Mark, MarkColor, MarkShape, MarkTool, MarkupSave, PageIndex, PageRect,
};

impl PdfMarkup {
    /// Everything written, to be saved.
    #[must_use]
    pub fn save(&self) -> MarkupSave {
        MarkupSave {
            fills: self
                .now
                .fills
                .iter()
                .map(|(name, value)| anyview_core::FieldFill {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            marks: self
                .now
                .marks
                .iter()
                .filter(|mark| self.differs_from_opened(mark))
                .cloned()
                .collect(),
            removed: self
                .opened
                .marks
                .iter()
                .filter(|held| {
                    !self
                        .now
                        .marks
                        .iter()
                        .any(|mark| mark.existing == held.existing)
                })
                .cloned()
                .collect(),
        }
    }

    /// Whether `mark` is one to write: made here, or held by the file and changed since.
    fn differs_from_opened(&self, mark: &Mark) -> bool {
        match mark.existing {
            None => true,
            Some(held) => self
                .opened
                .marks
                .iter()
                .find(|opened| opened.existing == Some(held))
                .is_none_or(|opened| opened != mark),
        }
    }

    /// Whether anything has been written that is not saved.
    #[must_use]
    pub fn is_edited(&self) -> bool {
        self.now != self.opened
    }

    /// Whether marks can be made.
    #[must_use]
    pub fn can_mark(&self) -> bool {
        self.marking == Allowance::Granted
    }

    /// Whether there is a change to take back.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is a change to do again.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The fields, a widget for each place one is drawn.
    #[must_use]
    pub fn widgets(&self) -> &[FormWidget] {
        &self.widgets
    }

    /// Whether the person can type or click in the widget at `index`.
    #[must_use]
    pub fn takes_input(&self, index: usize) -> bool {
        self.filling == Allowance::Granted
            && self.widgets.get(index).is_some_and(FormWidget::takes_input)
    }

    /// What the field of `widget` holds now: what the person wrote, else what the file holds.
    #[must_use]
    pub fn value_of<'a>(&'a self, widget: &'a FormWidget) -> &'a str {
        self.now
            .fills
            .get(&widget.name)
            .map_or(widget.value.as_str(), String::as_str)
    }

    /// Whether the check box or radio button at `index` is on.
    #[must_use]
    pub fn is_on(&self, index: usize) -> bool {
        self.widgets
            .get(index)
            .is_some_and(|widget| widget.kind.is_toggle() && self.value_of(widget) == widget.on)
    }

    /// The widget the keyboard is in.
    #[must_use]
    pub fn focus(&self) -> Option<usize> {
        self.focus
    }

    /// What the pointer does.
    #[must_use]
    pub fn using(&self) -> Using {
        self.using
    }

    /// What the Markup tool draws.
    #[must_use]
    pub fn tool(&self) -> MarkTool {
        self.tool
    }

    /// The colour of what is drawn next.
    #[must_use]
    pub fn color(&self) -> MarkColor {
        self.color
    }

    /// The marks made, in the order made (later ones are above earlier ones).
    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.now.marks
    }

    /// The mark picked.
    #[must_use]
    pub fn picked(&self) -> Option<usize> {
        self.picked
    }

    /// The note or text box being written in.
    #[must_use]
    pub fn writing(&self) -> Option<usize> {
        self.writing
    }

    /// The lines of the words selected on `page`.
    #[must_use]
    pub fn selection(&self, page: PageIndex) -> &[PageRect] {
        match &self.selected {
            Some(selected) if selected.page == page => &selected.lines,
            Some(_) | None => &[],
        }
    }

    /// Whether any words are selected.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selected.is_some()
    }

    /// The shape being drawn.
    #[must_use]
    pub fn draft(&self) -> Option<Draft> {
        match &self.drag {
            Some(Drag::Draw {
                page,
                tool,
                from,
                to,
                trail,
            }) => Some(Draft {
                page: *page,
                tool: *tool,
                from: *from,
                to: *to,
                trail: trail.clone(),
            }),
            Some(Drag::Move { .. }) | None => None,
        }
    }

    /// The words of the mark at `index` when it is a note or a text box.
    #[must_use]
    pub fn words_of(&self, index: usize) -> Option<&str> {
        match &self.now.marks.get(index)?.shape {
            MarkShape::Note { text, .. } | MarkShape::TextBox { text, .. } => Some(text),
            MarkShape::Highlight(_)
            | MarkShape::Underline(_)
            | MarkShape::Strikethrough(_)
            | MarkShape::Rectangle(_)
            | MarkShape::Oval(_)
            | MarkShape::Line { .. }
            | MarkShape::Arrow { .. }
            | MarkShape::Ink(_) => None,
        }
    }
}
