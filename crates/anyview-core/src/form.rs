//! The fields of a document's interactive form, as the pages show them: one widget for each place
//! a field is drawn, in the order the document gives them (the order the Tab key walks).

use crate::markup::PageRect;
use crate::units::PageIndex;
use ds_core::word::Word;

/// What a widget is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum WidgetKind {
    /// A line of text.
    Text,
    /// Several lines of text.
    #[word(label = "Text area")]
    TextArea,
    /// A line of text whose letters are not shown.
    Secret,
    /// A check box.
    Check,
    /// One button of a group of radio buttons.
    Radio,
    /// A drop-down list.
    Choice,
    /// A list of choices shown open.
    List,
}

impl WidgetKind {
    /// Whether the widget is a box that is on or off (a check box or a radio button).
    #[must_use]
    pub const fn is_toggle(self) -> bool {
        matches!(self, WidgetKind::Check | WidgetKind::Radio)
    }
}

/// Whether the person may change a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetAccess {
    /// It takes a value.
    Editable,
    /// The document keeps it as it is.
    ReadOnly,
}

/// One place on a page where a field is drawn.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormWidget {
    /// The field's fully qualified name; the widgets of a radio group share it.
    pub name: String,
    /// What it is.
    pub kind: WidgetKind,
    /// Whether it can be changed.
    pub access: WidgetAccess,
    /// The page it is on.
    pub page: PageIndex,
    /// Where it is on the page.
    pub rect: PageRect,
    /// What the field holds: the text, the chosen option, or the state name of the button that is
    /// on (`Off` when none is).
    pub value: String,
    /// For a check box or radio button, the state name that means this widget is on.
    pub on: String,
    /// For a choice, the options as the person reads them.
    pub options: Vec<String>,
    /// For a choice, the value written for each option, in the same order: the export value where
    /// the field has one, else the option itself.
    pub exports: Vec<String>,
}

impl FormWidget {
    /// Whether the person may type or click in it.
    #[must_use]
    pub fn takes_input(&self) -> bool {
        self.access == WidgetAccess::Editable
    }
}
