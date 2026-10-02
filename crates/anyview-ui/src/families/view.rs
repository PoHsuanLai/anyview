//! The full tier of one family of formats: what it opens, and how it draws, finds its controls
//! and fills the side panel. [`StageView`] is the trait each family implements once (the plan's
//! `Stage`, named for what it draws: the viewer's `Stage` is the machine enum of the stage
//! region); [`DocView`] is the loaded document seen without its family, which is what the window
//! holds, so the window needs no match on the family to draw it.

use crate::families::pdf::PdfShelf;
use crate::io::{HostRequest, Job, OpenError, OpenLink};
use crate::{Command, PanelParams, PanelTab, PanelTabs, Stage, StageIn, StageParams, Ticket};
use anyview_core::{Facts, LineIndex, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::prelude::{Point, Size};
use std::fmt::Debug;
use std::sync::Arc;

/// A loaded document of any family: shared, immutable, handed from the worker that opened it to
/// the window. Equality is identity, so a prop holding one re-renders only when the document does.
#[derive(Debug, Clone)]
pub struct LoadedDoc(Arc<dyn DocView>);

impl LoadedDoc {
    /// The document, seen without its family.
    pub(crate) fn view(&self) -> &dyn DocView {
        self.0.as_ref()
    }

    /// `doc` as a loaded document.
    pub(crate) fn of<S: StageView>(doc: S::Doc) -> LoadedDoc {
        LoadedDoc(Arc::new(Loaded::<S> { doc: Arc::new(doc) }))
    }
}

impl PartialEq for LoadedDoc {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
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
    /// The load this document belongs to.
    pub ticket: Ticket,
    /// The room, once measured.
    pub area: Option<Area>,
    /// An input for the stage machine.
    pub send: EventHandler<StageIn>,
    /// A command from a control the stage drew (the peek-only view's Open With…).
    pub run: EventHandler<Command>,
    /// The lines of a text file last read, newest window.
    pub lines: Option<Held<super::LineWindow>>,
    /// Ask for a window of lines: from this line, this many.
    pub ask_lines: EventHandler<(LineIndex, u32)>,
    /// Hand a job to the workers; its answer comes back through the window's mailbox.
    pub work: EventHandler<Job>,
    /// Ask the host to do something the viewer cannot (open a web address).
    pub request: EventHandler<HostRequest>,
    /// What the window holds of an open PDF: the tiles, the hits, the thumbnails.
    pub pdf: PdfShelf,
    /// The scheme the content is drawn in, for a sealed frame that cannot inherit it.
    pub frame: FrameLook,
}

/// What a sealed frame needs to wear the window's look: the root's attributes, written into the
/// frame's own document because a frame inherits nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameLook {
    /// The `class` and `data-*` attributes of the window's `Ds` root, as `name="value"` pairs.
    pub attributes: String,
}

/// One family of formats in the viewer's full tier.
pub trait StageView: 'static {
    /// The family of stage machine this draws.
    const FAMILY: crate::StageFamily;
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
    /// The rows the Info tab lists.
    fn facts(doc: &Self::Doc) -> Facts;
    /// The tabs the panel has for this document.
    fn tabs(doc: &Self::Doc) -> PanelTabs;
    /// What the stage machines are told of the document and the room: the scale the content is
    /// shown at, which views a text file has.
    fn params(doc: &Self::Doc, stage: &Stage, area: Option<Area>) -> StageParams;
    /// The content.
    fn stage(doc: &Arc<Self::Doc>, cx: &StageCx) -> Element;
    /// The capsule's controls, left to right.
    fn slots(
        doc: &Self::Doc,
        cx: &StageCx,
    ) -> Vec<ds::components::chrome::capsule::model::CapsuleSlot<Command>>;
    /// The panel's body for `tab`, when this family has something for it beyond the facts.
    fn panel(doc: &Arc<Self::Doc>, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    /// The job that reads `rows` lines from `first`, for a family that shows lines.
    fn lines(_doc: &Arc<Self::Doc>, _ticket: Ticket, _first: LineIndex, _rows: u32) -> Option<Job> {
        None
    }
}

/// A loaded document of any family, as the window reads it.
pub(crate) trait DocView: Debug + Send + Sync {
    fn facts(&self) -> Facts;
    fn panel_params(&self) -> PanelParams;
    fn params(&self, stage: &Stage, area: Option<Area>) -> StageParams;
    fn stage(&self, cx: &StageCx) -> Element;
    fn slots(&self, cx: &StageCx) -> Vec<CapsuleSlot<Command>>;
    fn panel(&self, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    fn lines(&self, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job>;
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

    fn params(&self, stage: &Stage, area: Option<Area>) -> StageParams {
        S::params(&self.doc, stage, area)
    }

    fn stage(&self, cx: &StageCx) -> Element {
        S::stage(&self.doc, cx)
    }

    fn slots(&self, cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
        S::slots(&self.doc, cx)
    }

    fn panel(&self, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        S::panel(&self.doc, tab, cx)
    }

    fn lines(&self, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job> {
        S::lines(&self.doc, ticket, first, rows)
    }
}
