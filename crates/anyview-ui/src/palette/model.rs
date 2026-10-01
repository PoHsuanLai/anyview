//! The palette's states, inputs and outputs.

use crate::command::Command;
use crate::typed::TypedText;
use ds_core::vocab::ShortcutKey;

/// A position in the ranked rows, from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RowIndex(pub usize);

/// Whether the palette is open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Palette {
    /// Not showing.
    #[default]
    Closed,
    /// Showing, with what was typed and the highlighted row.
    Open {
        query: TypedText,
        selection: RowIndex,
    },
}

/// Where the highlight goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMove {
    /// One row up.
    Up,
    /// One row down.
    Down,
    /// The first row.
    First,
    /// The last row.
    Last,
}

/// What moves the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteIn {
    /// Open with an empty query.
    Open,
    /// The field now holds this text; the rows are re-ranked for it.
    Typed(TypedText),
    /// Move the highlight; it stops at the first and last row.
    Move(PaletteMove),
    /// A row was clicked.
    Pick(RowIndex),
    /// Enter: run the highlighted row.
    Enter,
    /// Esc or a click outside.
    Close,
    /// The clock; the palette keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for PaletteIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        PaletteIn::Elapsed
    }
}

impl PaletteIn {
    /// What a key means to an open palette: arrows move, Enter runs, Esc and ⌘K close. Anything
    /// else is typed into the field, which reports it as `Typed`.
    pub fn from_key(keys: &[ShortcutKey]) -> Option<PaletteIn> {
        match keys {
            [ShortcutKey::Up] => Some(PaletteIn::Move(PaletteMove::Up)),
            [ShortcutKey::Down] => Some(PaletteIn::Move(PaletteMove::Down)),
            [ShortcutKey::Enter] => Some(PaletteIn::Enter),
            [ShortcutKey::Escape] | [ShortcutKey::Super, ShortcutKey::Char('k')] => {
                Some(PaletteIn::Close)
            }
            _ => None,
        }
    }
}

/// What the palette wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteOut {
    /// Show the palette and focus its field.
    Opened,
    /// Hide the palette and give focus back.
    Closed,
    /// Run this command.
    Run(Command),
}

/// The rows for the query now in the field, best first. Matching and ranking happen outside,
/// each time the query changes; the palette only walks the list it is given.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaletteParams {
    /// The ranked rows.
    pub rows: Vec<Command>,
}
