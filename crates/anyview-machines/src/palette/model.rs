//! The palette's states, inputs and outputs: quire's machine over [`Command`], and the scope that
//! anyview keeps beside it.

use crate::command::Command;
use crate::keys::{Act, Press};
use crate::typed::TypedText;
use ds_core::palette::model as base;
use ds_core::vocab::ShortcutKey;

pub use base::{PaletteIndex, PaletteMove};

/// Whether the palette is open, and its query and highlight: quire's palette over the viewer's
/// commands. What it lists (its [`PaletteScope`]) is the viewer's, beside it.
pub type Palette = base::PaletteState<Command>;

/// What the palette wants done; [`PaletteOut::Run`] carries the command to run.
pub type PaletteOut = base::PaletteOut<Command>;

/// The rows for the query now in the field, best first. Matching and ranking happen outside, each
/// time the query changes; the palette only walks the list it is given.
pub type PaletteParams = base::PaletteParams<Command>;

/// How many of a find's hits the palette lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HitList {
    /// The first few, with a row to list them all.
    #[default]
    Brief,
    /// Every one.
    Whole,
}

/// What the palette lists under its field: the commands the text names, or, as in mailo, the
/// places in the open file the text names and then the commands. Finding is the palette's, so there
/// is no find bar. It is `Commands` whenever the palette is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PaletteScope {
    /// The commands the text names (⌘K).
    #[default]
    Commands,
    /// The text is a find in the open file (⌘F): its hits, then the commands.
    Find(HitList),
}

/// What moves the palette: quire's inputs, and the two that make it a find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteIn {
    /// Open with an empty query.
    Open,
    /// Open as a find, on this text (the last find's, when there is one).
    OpenFind(TypedText),
    /// Make what is typed a find: ⌘F in the open palette, or the Find row.
    ToFind,
    /// The field now holds this text; the rows are re-ranked for it.
    Typed(TypedText),
    /// Move the highlight; it stops at the first and last row.
    Move(PaletteMove),
    /// A row was clicked.
    Pick(PaletteIndex),
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
    /// What a press means to an open palette: arrows move, Enter runs, Esc and the palette's own
    /// action close. Anything else is typed into the field, which reports it as `Typed`.
    pub fn from_press(press: &Press) -> Option<PaletteIn> {
        match press {
            Press::Key(shortcut) => match shortcut.keys().as_slice() {
                [ShortcutKey::Up] => Some(PaletteIn::Move(PaletteMove::Up)),
                [ShortcutKey::Down] => Some(PaletteIn::Move(PaletteMove::Down)),
                [ShortcutKey::Enter] => Some(PaletteIn::Enter),
                [ShortcutKey::Escape] => Some(PaletteIn::Close),
                _ => None,
            },
            Press::Act(Act::Palette) => Some(PaletteIn::Close),
            Press::Act(_) => None,
        }
    }
}
