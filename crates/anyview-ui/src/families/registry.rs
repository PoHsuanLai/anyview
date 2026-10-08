//! The stage registry: the one exhaustive match over `FormatKind` for the full tier. Adding a kind
//! is a compile error here until it names the view that shows it, and every kind without its own
//! view is mapped to [`PeekOnlyStageView`], never left out. `anyview_core::stage_support` says the same
//! thing in `anyview-core`'s own table; a test holds the two equal.

use super::media::MediaStageView;
use super::pdf::PdfStageView;
use super::peek_only::PeekOnlyStageView;
use super::raster::RasterStageView;
use super::table::TableStageView;
use super::text::TextStageView;
use super::tree::TreeStageView;
use super::view::{LoadedDoc, StageView};
use crate::io::{OpenError, OpenLink};
use crate::{LoadFlow, StageFamily, Ticket};
use anyview_core::{FormatKind, Sniffed, Source};

/// Something done with the view that shows a kind, without naming it.
pub trait KindVisitor {
    /// What it makes.
    type Out;
    /// Do it for the view `S`.
    fn visit<S: StageView>(self) -> Self::Out;
}

/// Run `visitor` for the view that shows `kind`.
pub fn visit<V: KindVisitor>(kind: FormatKind, visitor: V) -> V::Out {
    match kind {
        FormatKind::Raster | FormatKind::Vector => visitor.visit::<RasterStageView>(),
        FormatKind::Markdown | FormatKind::Code | FormatKind::PlainText => {
            visitor.visit::<TextStageView>()
        }
        FormatKind::Table => visitor.visit::<TableStageView>(),
        FormatKind::Tree => visitor.visit::<TreeStageView>(),
        FormatKind::Book | FormatKind::Pdf => visitor.visit::<PdfStageView>(),
        FormatKind::Video | FormatKind::Audio => visitor.visit::<MediaStageView>(),
        FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => visitor.visit::<PeekOnlyStageView>(),
    }
}

struct FamilyOf;

impl KindVisitor for FamilyOf {
    type Out = StageFamily;

    fn visit<S: StageView>(self) -> StageFamily {
        S::FAMILY
    }
}

/// The family of stage that shows `kind`.
pub fn family_of(kind: FormatKind) -> StageFamily {
    visit(kind, FamilyOf)
}

struct Opener<'a> {
    ticket: Ticket,
    src: &'a Source,
    sniffed: &'a Sniffed,
    link: &'a OpenLink,
}

impl KindVisitor for Opener<'_> {
    type Out = Result<LoadedDoc, OpenError>;

    fn visit<S: StageView>(self) -> Self::Out {
        S::open(self.ticket, self.src, self.sniffed, self.link).map(LoadedDoc::of::<S>)
    }
}

/// Open the file whose type `sniffed` established with the view that shows it. Blocking.
pub(crate) fn open_for(
    ticket: Ticket,
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<LoadedDoc, OpenError> {
    visit(
        sniffed.kind(),
        Opener {
            ticket,
            src,
            sniffed,
            link,
        },
    )
}

struct FlowOf;

impl KindVisitor for FlowOf {
    type Out = LoadFlow;

    fn visit<S: StageView>(self) -> LoadFlow {
        S::FLOW
    }
}

/// How a file of `kind` is opened: with a cheap first frame beside the open, or only the open.
pub fn flow_of(kind: FormatKind) -> LoadFlow {
    visit(kind, FlowOf)
}

struct FirstFrame<'a> {
    ticket: Ticket,
    src: &'a Source,
    sniffed: &'a Sniffed,
    link: &'a OpenLink,
}

impl KindVisitor for FirstFrame<'_> {
    type Out = Result<Option<LoadedDoc>, OpenError>;

    fn visit<S: StageView>(self) -> Self::Out {
        let doc = S::first_frame(self.ticket, self.src, self.sniffed, self.link)?;
        Ok(doc.map(LoadedDoc::of::<S>))
    }
}

/// The cheap first frame of the file whose type `sniffed` established, or `None` when it has no
/// frame cheaper than opening it. Blocking.
pub(crate) fn peek_for(
    ticket: Ticket,
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<Option<LoadedDoc>, OpenError> {
    visit(
        sniffed.kind(),
        FirstFrame {
            ticket,
            src,
            sniffed,
            link,
        },
    )
}
