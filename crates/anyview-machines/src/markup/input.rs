//! What changes the markup, and what it asks for.

use super::model::{Allowance, Using};
use anyview_core::{
    FormWidget, Mark, MarkColor, MarkTool, PageIndex, PagePoint, PageRect, Permille,
};
use std::sync::Arc;

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
        /// The annotations the file already holds that a mark can stand for, in the file's order,
        /// each with its [`Mark::existing`]. They are drawn, picked, moved and deleted like the
        /// marks made here, and a save writes only what was done to them.
        marks: Arc<[Mark]>,
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
