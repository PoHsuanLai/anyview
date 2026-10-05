//! The book stage's transitions.

use super::model::{BookIn, BookOut, BookParams, BookStage};
use anyview_core::{Resume, SectionIndex};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (BookStage, Vec<BookOut>);

impl Machine for BookStage {
    type In = BookIn;
    type Out = BookOut;
    type Params = BookParams;
    type Ctx = ();

    fn step(self, input: BookIn, _at: Stamp, params: &BookParams, _cx: &()) -> Step {
        let BookStage::Reading { section } = self;
        let before = self;
        let last = params.sections.last();
        let target = match input {
            BookIn::Elapsed => return (before, vec![]),
            BookIn::Next => SectionIndex(section.0.saturating_add(1)).min(last),
            BookIn::Previous => SectionIndex(section.0.saturating_sub(1)),
            BookIn::First => SectionIndex(0),
            BookIn::Last => last,
            BookIn::GoTo(section) => params.sections.clamp(section),
            BookIn::Restore(Resume::Book { section }) => {
                let section = params.sections.clamp(section);
                return (BookStage::Reading { section }, vec![]);
            }
            BookIn::Restore(
                Resume::Raster { .. }
                | Resume::Pdf { .. }
                | Resume::Media { .. }
                | Resume::Text { .. }
                | Resume::Nothing,
            ) => return (before, vec![]),
        };
        if target == section {
            return (before, vec![]);
        }
        (
            BookStage::Reading { section: target },
            vec![BookOut::Remember(Resume::Book { section: target })],
        )
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            BookStage::Reading { section: _ } => None,
        }
    }
}

impl BookStage {
    /// The section on screen.
    pub fn section(&self) -> SectionIndex {
        match self {
            BookStage::Reading { section } => *section,
        }
    }
}
