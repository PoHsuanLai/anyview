//! A loaded document seen without its family: what the window holds, so it needs no match on the
//! family to draw it.

use super::cx::{Area, HitLine, Leaving, StageCx};
use super::shared::EditDone;
use super::stage::StageView;
use crate::io::{Job, MediaLine, NaturalSize};
use crate::{
    Command, EditOffer, HitIndex, MediaOffer, PanelParams, PanelTab, Stage, StageIn, StageParams,
    Ticket, TypedText,
};
use anyview_core::{Facts, FindOptions, LineIndex, Resume};
use anyview_text::EditStamp;
use dioxus::prelude::*;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds_core::selection::RowSelection;
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
    #[must_use]
    pub fn facts(&self) -> Facts {
        self.view.facts().then(self.general.clone())
    }
}
impl PartialEq for LoadedDoc {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.view, &other.view)
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
        lines: Option<&super::super::LineWindow>,
    ) -> StageParams;
    fn arrived(&self, stage: &Stage, left_at: &Resume) -> Vec<StageIn>;
    fn natural(&self) -> Option<NaturalSize>;
    fn stage(&self, cx: &StageCx) -> Element;
    fn slots(&self, cx: &StageCx) -> Vec<RankedSlot<Command>>;
    fn panel(&self, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    fn modes(&self, cx: &StageCx) -> Option<Element>;
    fn lines(&self, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job>;
    fn edit_read(&self, ticket: Ticket) -> Option<Job>;
    fn search(
        &self,
        stage: &Stage,
        ticket: Ticket,
        query: &TypedText,
        options: FindOptions,
    ) -> Option<Job>;
    fn show_hit(
        &self,
        stage: &Stage,
        hits: &super::super::FoundHits,
        sorted: Option<&super::super::SortedOrder>,
        hit: HitIndex,
    ) -> Vec<StageIn>;
    fn copy_text(
        &self,
        stage: &Stage,
        rows: &RowSelection,
        sorted: Option<&super::super::SortedOrder>,
    ) -> Option<String>;
    fn edit(
        &self,
        ticket: Ticket,
        stage: &Stage,
        out: &crate::StageOut,
        order: Option<&super::super::SortedOrder>,
    ) -> EditDone;
    fn written(&self, stamp: EditStamp) -> Vec<StageIn>;
    fn hit_lines(&self, cx: &StageCx, upto: u32) -> Vec<HitLine>;
    fn leaving(&self) -> Leaving;
    fn line(&self) -> Option<Arc<dyn MediaLine>>;
    fn media_offer(&self) -> MediaOffer;
    fn edit_offer(&self) -> EditOffer;
    fn markup(&self) -> Option<crate::MarkupIn>;
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
        lines: Option<&super::super::LineWindow>,
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

    fn edit_read(&self, ticket: Ticket) -> Option<Job> {
        S::edit_read(&self.doc, ticket)
    }

    fn search(
        &self,
        stage: &Stage,
        ticket: Ticket,
        query: &TypedText,
        options: FindOptions,
    ) -> Option<Job> {
        S::search(&self.doc, stage, ticket, query, options)
    }

    fn show_hit(
        &self,
        stage: &Stage,
        hits: &super::super::FoundHits,
        sorted: Option<&super::super::SortedOrder>,
        hit: HitIndex,
    ) -> Vec<StageIn> {
        S::show_hit(&self.doc, stage, hits, sorted, hit)
    }

    fn copy_text(
        &self,
        stage: &Stage,
        rows: &RowSelection,
        sorted: Option<&super::super::SortedOrder>,
    ) -> Option<String> {
        S::copy_text(&self.doc, stage, rows, sorted)
    }

    fn edit(
        &self,
        ticket: Ticket,
        stage: &Stage,
        out: &crate::StageOut,
        order: Option<&super::super::SortedOrder>,
    ) -> EditDone {
        S::edit(&self.doc, ticket, stage, out, order)
    }

    fn written(&self, stamp: EditStamp) -> Vec<StageIn> {
        S::written(&self.doc, stamp)
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

    fn markup(&self) -> Option<crate::MarkupIn> {
        S::markup(&self.doc)
    }

    fn lacks(&self) -> Option<anyview_core::Helper> {
        S::lacks(&self.doc)
    }
}
