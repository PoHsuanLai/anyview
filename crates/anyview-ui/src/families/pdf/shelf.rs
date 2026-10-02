//! The window's handle on the live state of an open PDF: a signal the window makes once, writes
//! when a worker answers or the stage machine speaks, and the stage and the panel read.

use super::live::PdfLive;
use super::work::PdfAnswer;
use crate::{PdfOut, Stage, StageIn};
use dioxus::prelude::*;

/// The live state of the PDF the window shows. Copy: it is a signal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfShelf(Signal<PdfLive>);

/// The shelf of a window: made once, in the window's root.
pub fn use_pdf_shelf() -> PdfShelf {
    PdfShelf(use_signal(PdfLive::default))
}

impl PdfShelf {
    /// What the shelf holds, read where a change should redraw.
    pub(super) fn read(&self) -> impl std::ops::Deref<Target = PdfLive> + '_ {
        self.0.read()
    }

    /// What the shelf holds, read without subscribing.
    pub(super) fn peek(&self) -> impl std::ops::Deref<Target = PdfLive> + '_ {
        self.0.peek()
    }

    /// Change what the shelf holds.
    pub(super) fn with_mut<T>(&self, change: impl FnOnce(&mut PdfLive) -> T) -> T {
        let mut live = self.0;
        live.with_mut(change)
    }

    /// Forget the document: a new file is being loaded.
    pub fn reset(&self) {
        self.with_mut(|live| *live = PdfLive::default());
    }

    /// A worker answered. The input for the stage machine this calls for, if any.
    pub fn arrived(&self, answer: PdfAnswer, stage: &Stage) -> Option<StageIn> {
        self.with_mut(|live| live.received(answer, stage))
    }

    /// The PDF stage machine asked for something the view carries out: a scroll, a search, a hit.
    pub fn carry(&self, out: PdfOut) {
        self.with_mut(|live| live.carry(out));
    }
}
