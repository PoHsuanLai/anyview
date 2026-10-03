//! Keeping the mini window above the others, where the desktop lets a program do that.

use ds_core::word::Word;

/// Where the window sits among the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Stacking {
    /// Like any window.
    Normal,
    /// Above the others, even when another has focus.
    KeepAbove,
}

/// What a stacking request came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StackingOutcome {
    /// The window is stacked as asked.
    Applied,
    /// The desktop gives no way to ask; the window stays a normal one.
    Unsupported,
}

/// Ask for a stacking order.
pub trait WindowStacking {
    /// Ask for `stacking` for the mini window.
    fn request(&self, stacking: Stacking) -> StackingOutcome;
}
