//! The open PDF's markup: the state it keeps.

use anyview_core::{FormWidget, Mark, MarkColor, MarkTool, PageIndex, PagePoint, PageRect};
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
    /// What the document held when it was opened: its own annotations as marks, and no fills.
    /// `now` differs from it by what the person has written.
    pub(super) opened: Written,
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
            opened: Written::default(),
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
