//! The context menu's states, inputs, outputs and parameters.

use crate::command::Command;

/// A point in the window, in whole logical pixels from its top-left corner. Whole pixels keep the
/// state `Eq`, and a menu is not placed finer than a pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Spot {
    /// Pixels from the left edge.
    pub x: i32,
    /// Pixels from the top edge.
    pub y: i32,
}

/// Whether the context menu is open, and where its corner is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextMenu {
    /// Not showing.
    #[default]
    Closed,
    /// Showing, with its top-left corner at `at`.
    Open { at: Spot },
}

/// What a context menu row does when it is picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextPick {
    /// Run a command the palette lists too.
    Run(Command),
    /// Show the file's facts in the side panel.
    GetInfo,
}

/// One line of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextEntry {
    /// A row, named as a Mac menu names it.
    Item {
        pick: ContextPick,
        title: &'static str,
    },
    /// A rule between two groups.
    Separator,
}

/// What moves the context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextIn {
    /// A secondary click: open at the pointer.
    Open(Spot),
    /// The Menu key or ⇧F10: open at the middle of the content.
    OpenAtCentre,
    /// A row was picked: it runs, and the menu stays up until it has faded out and closes.
    Pick(ContextPick),
    /// Esc, a click outside, or the end of the fade after a pick.
    Close,
    /// The clock; the menu keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for ContextIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        ContextIn::Elapsed
    }
}

/// What the context menu wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextOut {
    /// Do what this row says.
    Run(ContextPick),
}

/// What the menu reads: the rows the open file has now, and where the middle of the content is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextParams {
    /// The rows, in order; empty when the file offers nothing, and then no menu opens.
    pub entries: Vec<ContextEntry>,
    /// Where a menu opened by a key goes.
    pub centre: Spot,
}
