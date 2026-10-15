//! The open PDF's markup: its state, its inputs and what it asks for.

use anyview_core::{
    FormWidget, Mark, MarkColor, MarkShape, MarkTool, MarkupSave, PageIndex, PagePoint, PageRect,
    Permille,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Whether the document lets the person do something to it: a protected file may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Allowance {
    /// It does.
    Granted,
    /// It does not, and nothing that would is offered.
    #[default]
    Withheld,
}

/// What the pointer does on the pages: the pointer tool, as the markup hears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Using {
    /// A drag moves the pages; the pointer writes nothing.
    #[default]
    Reading,
    /// A drag selects the words under it.
    Selecting,
    /// The Markup tool is on: the pointer draws and picks marks.
    Marking,
}

/// What has been written: the fields filled, by name, and the marks, in the order made.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Written {
    pub(super) fills: BTreeMap<String, String>,
    pub(super) marks: Vec<Mark>,
}

/// Whether the last change was a letter typed into a field, so that the next one in the same field
/// is the same step to take back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum Typing {
    #[default]
    Idle,
    Into(String),
}

/// A drag that is under way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Drag {
    /// A shape being drawn from `from`, on `page`.
    Draw {
        page: PageIndex,
        tool: MarkTool,
        from: PagePoint,
        to: PagePoint,
        trail: Vec<PagePoint>,
    },
    /// The mark at `index` being moved: where the pointer took hold of it, and what was written
    /// before it moved.
    Move {
        index: usize,
        from: PagePoint,
        base: Written,
    },
}

/// A shape on its way to being a mark, as the view draws it while the pointer is down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The page it is on.
    pub page: PageIndex,
    /// What it will be.
    pub tool: MarkTool,
    /// Where the pointer went down.
    pub from: PagePoint,
    /// Where it is now.
    pub to: PagePoint,
    /// The points the pointer has gone through, for a sketch; empty for any other tool.
    pub trail: Vec<PagePoint>,
}

/// The words selected on a page, as the lines they make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Selected {
    pub(super) page: PageIndex,
    pub(super) lines: Vec<PageRect>,
}

/// The open PDF's markup. It is about the document as it was opened, so a document landing starts
/// it afresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfMarkup {
    pub(super) widgets: Arc<[FormWidget]>,
    pub(super) filling: Allowance,
    pub(super) marking: Allowance,
    pub(super) now: Written,
    pub(super) undo: Vec<Written>,
    pub(super) redo: Vec<Written>,
    pub(super) typing: Typing,
    pub(super) focus: Option<usize>,
    pub(super) using: Using,
    pub(super) tool: MarkTool,
    pub(super) color: MarkColor,
    pub(super) selected: Option<Selected>,
    pub(super) drag: Option<Drag>,
    pub(super) picked: Option<usize>,
    pub(super) writing: Option<usize>,
}

impl Default for PdfMarkup {
    fn default() -> Self {
        PdfMarkup {
            widgets: Arc::from(Vec::new()),
            filling: Allowance::Withheld,
            marking: Allowance::Withheld,
            now: Written::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            typing: Typing::Idle,
            focus: None,
            using: Using::Reading,
            tool: MarkTool::Highlight,
            color: MarkColor::Yellow,
            selected: None,
            drag: None,
            picked: None,
            writing: None,
        }
    }
}

/// What changes the markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkupIn {
    /// A document is on screen (or the file was opened again after a save): where its fields are,
    /// and what it lets the person do. Whatever was written on the last one is gone.
    Landed {
        /// The fields, a widget for each place one is drawn, in the document's order.
        widgets: Arc<[FormWidget]>,
        /// Whether the form can be filled.
        filling: Allowance,
        /// Whether marks can be made.
        marking: Allowance,
    },
    /// The pointer tool changed.
    Using(Using),
    /// The keyboard went into this field (a press on it, or Tab).
    Focus(usize),
    /// The keyboard left this field.
    Blur(usize),
    /// Tab: the next field that takes a value, in the document's order, after the one the keyboard
    /// is in; the first when it is in none.
    Next,
    /// ⇧Tab: the field before.
    Previous,
    /// The whole text of a field is now this (a text field, a choice).
    Type {
        /// The widget the person typed in.
        widget: usize,
        /// What it holds now.
        text: String,
    },
    /// A press on a check box or radio button.
    Toggle(usize),
    /// Choose what the pointer draws.
    Tool(MarkTool),
    /// Choose the colour of what is drawn next, and of the mark that is picked.
    Color(MarkColor),
    /// These lines of text are selected on a page, or nothing is.
    Select(Option<(PageIndex, Vec<PageRect>)>),
    /// Mark the selected words with a highlight, underline or strike-through.
    Apply(MarkTool),
    /// The pointer went down on a page, at this point of it. `reach` is how near a mark's ink
    /// counts as on it.
    Press {
        /// The page.
        page: PageIndex,
        /// Where.
        at: PagePoint,
        /// How near, in thousandths of the page.
        reach: u32,
    },
    /// The pointer moved with the button down, to this point of the page it went down on.
    Drag(PagePoint),
    /// The pointer was let go.
    Release,
    /// Type in the note or text box being written: this is all its words now.
    Words(String),
    /// Begin writing in the mark at this index (a double press on a note or a text box).
    Write(usize),
    /// Stop writing; a note or text box left empty is taken back.
    Done,
    /// Take the picked mark away.
    Delete,
    /// Esc: put away what is selected, picked or being written.
    Clear,
    /// Let everything written go: the document is as it was opened.
    Discard,
    /// Take back the last change.
    Undo,
    /// Do again the change that was taken back.
    Redo,
}

/// What the markup asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkupOut {
    /// Show this place: the field the keyboard moved to is not on the page in view.
    Reveal {
        /// The page.
        page: PageIndex,
        /// How far down it, in thousandths of its height.
        offset: Permille,
    },
}

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
            marks: self.now.marks.clone(),
        }
    }

    /// Whether anything has been written that is not saved.
    #[must_use]
    pub fn is_edited(&self) -> bool {
        self.now != Written::default()
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
