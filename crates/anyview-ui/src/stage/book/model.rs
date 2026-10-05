//! The book stage's states, inputs and outputs: a chapter or a comic page on screen, and the
//! moves between them.

use anyview_core::{Resume, SectionCount, SectionIndex};

/// What the book stage is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookStage {
    /// Reading `section`.
    Reading { section: SectionIndex },
}

impl Default for BookStage {
    fn default() -> Self {
        BookStage::Reading {
            section: SectionIndex(0),
        }
    }
}

/// What moves the stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookIn {
    /// The next section; the last stays.
    Next,
    /// The previous section; the first stays.
    Previous,
    /// The first section.
    First,
    /// The last section.
    Last,
    /// This section, or the last when it is past the end.
    GoTo(SectionIndex),
    /// Put the place the file was left at back.
    Restore(Resume),
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for BookIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        BookIn::Elapsed
    }
}

/// What the stage asks of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookOut {
    /// Keep this place for next time.
    Remember(Resume),
}

/// What the stage is told of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookParams {
    /// How many sections the book has.
    pub sections: SectionCount,
}

impl Default for BookParams {
    fn default() -> Self {
        BookParams {
            sections: SectionCount::new(1).unwrap_or_else(|| unreachable!("one is not zero")),
        }
    }
}
