//! Edits: each change the viewer can save in place, described as data.

mod adjust;
#[cfg(test)]
mod tests;

pub use adjust::{Adjust, PixelRect, Reflection};

use crate::units::{Axis, PageIndex, PageRange, QuarterTurn, TimeRange};
use ds_core::word::Word;

/// One change to a file. An edit is a value: the save pipeline applies it, undo stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Edit {
    /// Turn the content clockwise by this much.
    Rotate(QuarterTurn),
    /// Mirror the content across an axis.
    Flip(Axis),
    /// Remove a run of pages.
    DeletePages(PageRange),
    /// Move one page to a new place.
    MovePage {
        /// The page's place now.
        from: PageIndex,
        /// The place it moves to.
        to: PageIndex,
    },
    /// Everything done to a picture since it was opened, as one change: cut down, mirrored,
    /// turned and resized.
    Adjust(Adjust),
    /// Keep one stretch of a recording and drop the rest. The range cannot end before it starts,
    /// also when it is read back from a store.
    Trim(TimeRange),
    /// Everything written on a document's pages since it was opened, as one change: the form
    /// fields filled and the marks made. What was written is the viewer's, not the edit's.
    Markup,
}

/// The sorts of edit, without their options: what a kind of file offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum EditKind {
    /// [`Edit::Rotate`].
    Rotate,
    /// [`Edit::Flip`].
    Flip,
    /// [`Edit::DeletePages`].
    DeletePages,
    /// [`Edit::MovePage`].
    MovePage,
    /// [`Edit::Adjust`].
    Adjust,
    /// [`Edit::Trim`].
    Trim,
    /// [`Edit::Markup`].
    Markup,
}

impl Edit {
    /// Which sort of edit this is.
    pub fn kind(&self) -> EditKind {
        match self {
            Edit::Rotate(_) => EditKind::Rotate,
            Edit::Flip(_) => EditKind::Flip,
            Edit::DeletePages(_) => EditKind::DeletePages,
            Edit::MovePage { .. } => EditKind::MovePage,
            Edit::Adjust(_) => EditKind::Adjust,
            Edit::Trim(_) => EditKind::Trim,
            Edit::Markup => EditKind::Markup,
        }
    }
}
