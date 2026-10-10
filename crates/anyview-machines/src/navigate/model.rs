//! Navigation's states, inputs and outputs.

use anyview_core::{FilePath, Heading, Neighbours, Sequence};
use ds_core::vocab::ShortcutKey;

/// Whether there is a list to walk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Navigate {
    /// A single file with no list around it.
    #[default]
    Idle,
    /// Walking `sequence`; its position is the open file, and `heading` the way the last move went.
    Walking {
        sequence: Sequence,
        heading: Heading,
    },
}

/// What moves navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigateIn {
    /// The open file belongs to this list.
    Start(Sequence),
    /// → : the next file. The walk stops at the end.
    Next,
    /// ← : the previous file. The walk stops at the start.
    Previous,
    /// Home: the first file.
    First,
    /// End: the last file.
    Last,
    /// The open file is no longer one of the list (another file was dropped on the window): there
    /// is nothing to walk until a list for the new file arrives.
    Leave,
    /// The open file is gone from disk and there are others in the list: leave it out of the walk
    /// and open the one the person was heading for.
    Gone,
    /// The clock; navigation keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for NavigateIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        NavigateIn::Elapsed
    }
}

impl NavigateIn {
    /// What a key means to navigation: the arrows and Home and End.
    pub fn from_key(keys: &[ShortcutKey]) -> Option<NavigateIn> {
        match keys {
            [ShortcutKey::Right] => Some(NavigateIn::Next),
            [ShortcutKey::Left] => Some(NavigateIn::Previous),
            [ShortcutKey::Home] => Some(NavigateIn::First),
            [ShortcutKey::End] => Some(NavigateIn::Last),
            _ => None,
        }
    }
}

/// What navigation wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigateOut {
    /// Open this file: the walk moved onto it.
    Open(FilePath),
    /// Load these neighbours of the open file ahead of time.
    Preload(Neighbours),
}
