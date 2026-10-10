//! The full tier of one family of formats: what it opens, and how it draws, finds its controls
//! and fills the side panel. [`StageView`] is the trait each family implements once (the plan's
//! `Stage`, named for what it draws: the viewer's `Stage` is the machine enum of the stage
//! region); [`DocView`] is the loaded document seen without its family, which is what the window
//! holds, so the window needs no match on the family to draw it.

use crate::families::media::MediaShelf;
use crate::families::pdf::PdfShelf;
use crate::io::{HostRequest, Job, MediaLine, NaturalSize, OpenError, OpenLink};
use crate::{
    Command, EditOffer, LoadFlow, MediaOffer, PanelParams, PanelTab, PanelTabs, Stage, StageIn,
    StageParams, Ticket, TypedText,
};
use anyview_core::{Facts, LineIndex, Resume, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::prelude::{Point, Size};
use std::fmt::Debug;
use std::sync::Arc;

/// A loaded document of any family: shared, immutable, handed from the worker that opened it to
/// the window. Equality is identity, so a prop holding one re-renders only when the document does.
#[derive(Debug, Clone)]
pub struct LoadedDoc {
    view: Arc<dyn DocView>,
    general: Facts,
}

impl LoadedDoc {
    /// The document, seen without its family.
    pub(crate) fn view(&self) -> &dyn DocView {
        self.view.as_ref()
    }

    /// `doc` as a loaded document, with no General section: a folder preloaded for the next
    /// arrow press has no file to describe.
    pub(crate) fn of<S: StageView>(doc: S::Doc) -> LoadedDoc {
        LoadedDoc {
            view: Arc::new(Loaded::<S> { doc: Arc::new(doc) }),
            general: Facts::empty(),
        }
    }

    /// This document, whose file the General section `general` describes (what any file has:
    /// kind, size, dates, where it is). Read once, on the worker that opened the file.
    pub(crate) fn describing(self, general: Facts) -> LoadedDoc {
        LoadedDoc { general, ..self }
    }

    /// The rows of the Info tab: the family's own, then the General section. A family row the
    /// General section also has (its kind as a media type, its size) gives way to it.
    pub fn facts(&self) -> Facts {
        self.view.facts().then(self.general.clone())
    }
}

impl PartialEq for LoadedDoc {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.view, &other.view)
    }
}

/// A shared piece of a document or window state that compares by identity.
#[derive(Debug)]
pub struct Held<T>(pub Arc<T>);

impl<T> Clone for Held<T> {
    fn clone(&self) -> Self {
        Held(Arc::clone(&self.0))
    }
}

impl<T> PartialEq for Held<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Thousandths of zoom a logical pixel of a wheel turn under Control makes.
pub(crate) const WHEEL_ZOOM: f32 = 5.0;

/// The room the content has, as the window measured it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// The content box's top-left corner in the window, in logical pixels.
    pub origin: Point,
    /// The content box's size, in logical pixels.
    pub size: Size,
    /// Device pixels per logical pixel.
    pub scale: f32,
}

/// What a family's views read from the window and send back to it. Everything is a value or a
/// callback, so a view decides nothing the machines decide.
#[derive(Debug, Clone, PartialEq)]
pub struct StageCx {
    /// The stage machine's state.
    pub stage: Stage,
    /// The pan tool: whether a drag on a picture pans it.
    pub hand: crate::Hand,
    /// The load this document belongs to.
    pub ticket: Ticket,
    /// The room, once measured.
    pub area: Option<Area>,
    /// An input for the stage machine.
    pub send: EventHandler<StageIn>,
    /// A command from a control the stage drew (the peek-only view's Show in Folder).
    pub run: EventHandler<Command>,
    /// The lines of a text file last read, newest window.
    pub lines: Option<Held<super::LineWindow>>,
    /// Ask for a window of lines: from this line, this many.
    pub ask_lines: EventHandler<(LineIndex, u32)>,
    /// The places the current find found, when one is up.
    pub hits: Option<Held<super::FoundHits>>,
    /// Hand a job to the workers; its answer comes back through the window's mailbox.
    pub work: EventHandler<Job>,
    /// Ask the host to do something the viewer cannot (open a web address).
    pub request: EventHandler<HostRequest>,
    /// What the window holds of an open PDF: the tiles, the hits, the thumbnails.
    pub pdf: PdfShelf,
    /// What the window holds of the recording that plays: the position, the lists.
    pub media: MediaShelf,
    /// The scheme the content is drawn in, for a sealed frame that cannot inherit it.
    pub frame: FrameLook,
    /// What the platform can do: a control of a service it lacks is not drawn.
    pub platform: crate::PlatformAbilities,
}

/// One place a find found, as the palette lists it under "In This File": the words around the match
/// with the match marked, and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HitLine {
    /// The words around the match (a line of text, a snippet).
    pub context: String,
    /// The part of `context` the find matched, as byte offsets into it.
    pub matched: std::ops::Range<usize>,
    /// Where it is, as the person counts: `Line 42`, `Page 7`.
    pub place: String,
}

/// What a sealed frame needs to wear the window's look: the root's attributes, written into the
/// frame's own document because a frame inherits nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameLook {
    /// The `class` and `data-*` attributes of the window's `Ds` root, as `name="value"` pairs.
    pub attributes: String,
}

/// What becomes of a document when the person walks away from its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaving {
    /// It is kept as it was left, so coming back to the file needs no open.
    Keep,
    /// It is let go: a recording that kept its player would go on playing unseen.
    Release,
}

/// One family of formats in the viewer's full tier.
pub trait StageView: 'static {
    /// The family of stage machine this draws.
    const FAMILY: crate::StageFamily;
    /// Whether a file of this family has a first frame cheaper than the full open.
    const FLOW: LoadFlow = LoadFlow::OpenOnly;
    /// What happens to the document when the person leaves its file.
    const LEAVING: Leaving = Leaving::Keep;
    /// What opening a file makes: the decoded picture, the text index.
    type Doc: Debug + Send + Sync + 'static;

    /// Open the file whose type `sniffed` established. Blocking: runs on a worker, and uploads
    /// what it can into `link` itself.
    fn open(
        ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<Self::Doc, OpenError>;
    /// The cheap first frame of the file, shown while `open` runs, or `None` when there is not one
    /// for this file. Blocking, on a worker; only called for a family whose `FLOW` is
    /// `PeekThenOpen`.
    fn first_frame(
        _ticket: Ticket,
        _src: &Source,
        _sniffed: &Sniffed,
        _link: &OpenLink,
    ) -> Result<Option<Self::Doc>, OpenError> {
        Ok(None)
    }
    /// The rows the Info tab lists.
    fn facts(doc: &Self::Doc) -> Facts;
    /// The tabs the panel has for this document.
    fn tabs(doc: &Self::Doc) -> PanelTabs;
    /// What the stage machines are told of the document and the room: the scale the content is
    /// shown at, which views a text file has.
    fn params(doc: &Self::Doc, stage: &Stage, area: Option<Area>) -> StageParams;
    /// `params` for a family whose parameters depend on the lines last read (how many of them fit
    /// a page once long lines wrap); the others need not say.
    fn params_seen(
        doc: &Self::Doc,
        stage: &Stage,
        area: Option<Area>,
        _lines: Option<&super::LineWindow>,
    ) -> StageParams {
        Self::params(doc, stage, area)
    }
    /// The inputs the stage is told when `doc` lands, given the stage that is showing and where
    /// the file was left (`Probed::resume`): an animation says it moves, a find already up is
    /// asked again of the new copy, a place that needs the document's extent is put back.
    fn arrived(_doc: &Self::Doc, _stage: &Stage, _left_at: &Resume) -> Vec<StageIn> {
        Vec::new()
    }
    /// How big the content naturally is, once loaded, when only the loaded document knows (a
    /// PDF's first page at 100%, a picture a plugin decoded): what the window is sized to after
    /// its first file has loaded.
    fn natural(_doc: &Self::Doc) -> Option<NaturalSize> {
        None
    }
    /// The content.
    fn stage(doc: &Arc<Self::Doc>, cx: &StageCx) -> Element;
    /// The capsule's controls, left to right.
    fn slots(
        doc: &Self::Doc,
        cx: &StageCx,
    ) -> Vec<ds::components::chrome::capsule::priority::RankedSlot<Command>>;
    /// The panel's body for `tab`, when this family has something for it beyond the facts.
    fn panel(doc: &Arc<Self::Doc>, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    /// The controls on the titlebar's trailing side, for a family with a mode to choose (a
    /// picture's Select | Pan). The titlebar fades and returns with the capsule.
    fn modes(_doc: &Arc<Self::Doc>, _cx: &StageCx) -> Option<Element> {
        None
    }
    /// The job that reads `rows` lines from `first`, for a family that shows lines.
    fn lines(_doc: &Arc<Self::Doc>, _ticket: Ticket, _first: LineIndex, _rows: u32) -> Option<Job> {
        None
    }
    /// The job that finds `query` in the document, for a family that can search.
    fn search(_doc: &Arc<Self::Doc>, _ticket: Ticket, _query: &TypedText) -> Option<Job> {
        None
    }
    /// The first `upto` hits of the find that is up, as the palette lists them, for a family that
    /// can search.
    fn hit_lines(_doc: &Arc<Self::Doc>, _cx: &StageCx, _upto: u32) -> Vec<HitLine> {
        Vec::new()
    }
    /// The line to the player, for a family that plays.
    fn line(_doc: &Self::Doc) -> Option<Arc<dyn MediaLine>> {
        None
    }
    /// The media exports on offer, for a family that has them.
    fn media_offer(_doc: &Self::Doc) -> MediaOffer {
        MediaOffer::default()
    }
    /// What an edit of the document costs, for a family whose files are edited in place.
    fn edit_offer(_doc: &Self::Doc) -> EditOffer {
        EditOffer::Plain
    }
    /// The tool the document is shown without because the system lacks it, when installing it
    /// would let the file show fully.
    fn lacks(_doc: &Self::Doc) -> Option<anyview_core::Helper> {
        None
    }
}

/// A loaded document of any family, as the window reads it.
pub(crate) trait DocView: Debug + Send + Sync {
    fn facts(&self) -> Facts;
    fn panel_params(&self) -> PanelParams;
    fn params(
        &self,
        stage: &Stage,
        area: Option<Area>,
        lines: Option<&super::LineWindow>,
    ) -> StageParams;
    fn arrived(&self, stage: &Stage, left_at: &Resume) -> Vec<StageIn>;
    fn natural(&self) -> Option<NaturalSize>;
    fn stage(&self, cx: &StageCx) -> Element;
    fn slots(&self, cx: &StageCx) -> Vec<RankedSlot<Command>>;
    fn panel(&self, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    fn modes(&self, cx: &StageCx) -> Option<Element>;
    fn lines(&self, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job>;
    fn search(&self, ticket: Ticket, query: &TypedText) -> Option<Job>;
    fn hit_lines(&self, cx: &StageCx, upto: u32) -> Vec<HitLine>;
    fn leaving(&self) -> Leaving;
    fn line(&self) -> Option<Arc<dyn MediaLine>>;
    fn media_offer(&self) -> MediaOffer;
    fn edit_offer(&self) -> EditOffer;
    fn lacks(&self) -> Option<anyview_core::Helper>;
}

/// A document of family `S`, which is how it knows how to draw itself.
struct Loaded<S: StageView> {
    doc: Arc<S::Doc>,
}

impl<S: StageView> Debug for Loaded<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.doc.fmt(f)
    }
}

impl<S: StageView> DocView for Loaded<S> {
    fn facts(&self) -> Facts {
        S::facts(&self.doc)
    }

    fn panel_params(&self) -> PanelParams {
        PanelParams {
            tabs: S::tabs(&self.doc),
        }
    }

    fn params(
        &self,
        stage: &Stage,
        area: Option<Area>,
        lines: Option<&super::LineWindow>,
    ) -> StageParams {
        S::params_seen(&self.doc, stage, area, lines)
    }

    fn arrived(&self, stage: &Stage, left_at: &Resume) -> Vec<StageIn> {
        S::arrived(&self.doc, stage, left_at)
    }

    fn natural(&self) -> Option<NaturalSize> {
        S::natural(&self.doc)
    }

    fn stage(&self, cx: &StageCx) -> Element {
        S::stage(&self.doc, cx)
    }

    fn slots(&self, cx: &StageCx) -> Vec<RankedSlot<Command>> {
        S::slots(&self.doc, cx)
    }

    fn panel(&self, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        S::panel(&self.doc, tab, cx)
    }

    fn modes(&self, cx: &StageCx) -> Option<Element> {
        S::modes(&self.doc, cx)
    }

    fn lines(&self, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job> {
        S::lines(&self.doc, ticket, first, rows)
    }

    fn search(&self, ticket: Ticket, query: &TypedText) -> Option<Job> {
        S::search(&self.doc, ticket, query)
    }

    fn hit_lines(&self, cx: &StageCx, upto: u32) -> Vec<HitLine> {
        S::hit_lines(&self.doc, cx, upto)
    }

    fn leaving(&self) -> Leaving {
        S::LEAVING
    }

    fn line(&self) -> Option<Arc<dyn MediaLine>> {
        S::line(&self.doc)
    }

    fn media_offer(&self) -> MediaOffer {
        S::media_offer(&self.doc)
    }

    fn edit_offer(&self) -> EditOffer {
        S::edit_offer(&self.doc)
    }

    fn lacks(&self) -> Option<anyview_core::Helper> {
        S::lacks(&self.doc)
    }
}
