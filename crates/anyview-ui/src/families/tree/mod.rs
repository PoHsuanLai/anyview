//! JSON and JSON Lines: the tree stage's view. A file is scanned on a worker (`doc`), which indexes it
//! without holding it; the rows on screen are the visible nodes in a `VirtualList`, read a window at a
//! time.

mod doc;
mod edits;
mod find;
mod view;

pub use doc::TreeDoc;
pub(crate) use find::search_tree;

use crate::families::FoundHits;
use crate::families::SortedOrder;
use crate::families::view::{Area, EditDone, Held, HitLine, StageCx, StageView};
use crate::io::{Job, OpenError, OpenPort};
use crate::{
    Command, ContentScale, EditOffer, Editable, HitIndex, KeyUnder, NodeUnder, PanelTab, PanelTabs,
    Stage, StageCommand, StageFamily, StageIn, StageOut, StageParams, Ticket, TreeIn, TreeParams,
    TypedText,
};
use anyview_core::work::Stop;
use anyview_core::{Facts, FindOptions, Resume, Sniffed, Source};
use anyview_text::{Coverage, EditStamp, NodeKind, RowLabel};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::{RankedSlot, essentials};
use ds::prelude::Icon;
use std::sync::Arc;

/// JSON and JSON Lines files, shown as a tree that opens node by node.
#[derive(Debug, Clone, Copy)]
pub struct TreeStageView;

impl StageView for TreeStageView {
    const FAMILY: StageFamily = StageFamily::Tree;
    type Doc = TreeDoc;

    fn open(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        _link: &OpenPort,
        stop: &Stop,
    ) -> Result<TreeDoc, OpenError> {
        doc::open(src, sniffed, stop)
    }

    fn facts(doc: &TreeDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(_doc: &TreeDoc) -> PanelTabs {
        PanelTabs::of(&[PanelTab::Info])
    }

    fn params(doc: &TreeDoc, stage: &Stage, area: Option<Area>) -> StageParams {
        let tree = doc.now();
        let rows = match stage {
            Stage::Tree(stage) => tree.visible_count(stage.open()),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Table(_) => 0,
        };
        let scale = match stage {
            Stage::Tree(stage) => stage.scale(),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Table(_) => ContentScale::ACTUAL,
        };
        let (under, key) = under_cursor(&tree, stage);
        StageParams {
            tree: TreeParams {
                rows,
                page: view::page_of(area, scale),
                editable: if doc.is_editable() {
                    Editable::Yes
                } else {
                    Editable::No
                },
                under,
                key,
            },
            ..StageParams::default()
        }
    }

    fn arrived(doc: &TreeDoc, stage: &Stage, _left_at: &Resume) -> Vec<StageIn> {
        // What the stage holds of the edits is what the document that has landed holds.
        let held = doc.session();
        let log = held.as_ref().map_or(crate::EditLog::NONE, edits::log_of);
        let mut told = vec![StageIn::Tree(TreeIn::Session(
            crate::EditSessionIn::Logged(log),
        ))];
        // A find that was up is asked again of the document that has landed.
        if let Stage::Tree(tree) = stage
            && let Some(find) = tree.find()
        {
            told.push(StageIn::Tree(TreeIn::Find(find.query.clone())));
        }
        told
    }

    fn edit(
        doc: &Arc<TreeDoc>,
        _ticket: Ticket,
        stage: &Stage,
        out: &StageOut,
        _order: Option<&SortedOrder>,
    ) -> EditDone {
        edits::carry(doc, stage, out)
    }

    fn written(doc: &TreeDoc, stamp: EditStamp) -> Vec<StageIn> {
        let mut held = doc.session();
        match held.as_mut() {
            Some(session) => {
                session.mark_saved(stamp);
                vec![StageIn::Tree(TreeIn::Session(
                    crate::EditSessionIn::Logged(edits::log_of(session)),
                ))]
            }
            None => Vec::new(),
        }
    }

    fn edit_offer(doc: &TreeDoc) -> EditOffer {
        if doc.is_editable() {
            EditOffer::Plain
        } else {
            EditOffer::Withheld
        }
    }

    fn stage(doc: &Arc<TreeDoc>, cx: &StageCx) -> Element {
        rsx! { view::TreeContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(_doc: &TreeDoc, cx: &StageCx) -> Vec<RankedSlot<Command>> {
        let mut slots = essentials(vec![CapsuleSlot::button(
            Command::Stage(StageCommand::CollapseAll),
            "Collapse All",
            // The set has no inward pair of chevrons (collapse all); the outward pair is its fold
            // mark, which a tree's person reads as the folding control, where one chevron up
            // read as "scroll to top".
            Icon::ChevronsUpDown,
        )]);
        slots.extend(crate::families::found::standing(&cx.stage));
        slots
    }

    fn search(
        doc: &Arc<TreeDoc>,
        _stage: &Stage,
        ticket: Ticket,
        query: &TypedText,
        options: FindOptions,
    ) -> Option<Job> {
        Some(Job::SearchNodes {
            ticket,
            doc: Arc::clone(doc),
            query: query.clone(),
            options,
        })
    }

    fn show_hit(
        doc: &Arc<TreeDoc>,
        stage: &Stage,
        hits: &FoundHits,
        _sorted: Option<&SortedOrder>,
        hit: HitIndex,
    ) -> Vec<StageIn> {
        find::show(doc, stage, hits, hit)
    }

    fn hit_lines(_doc: &Arc<TreeDoc>, cx: &StageCx, upto: u32) -> Vec<HitLine> {
        match cx.hits.as_ref() {
            Some(found) => find::lines(&found.0, upto),
            None => Vec::new(),
        }
    }

    fn panel(doc: &Arc<TreeDoc>, tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        match (tab, doc.coverage) {
            // The panel's footer says the file runs on past what the tree holds.
            (PanelTab::Info, Coverage::Prefix) => Some(rsx! {
                p { class: "viewer-panel-note", "Showing the start of the file" }
            }),
            (PanelTab::Info, Coverage::Whole)
            | (
                PanelTab::Thumbnails | PanelTab::Contents | PanelTab::Sheets | PanelTab::Tracks,
                Coverage::Whole | Coverage::Prefix,
            ) => None,
        }
    }
}

/// What the node under the cursor holds and whether it has a key, as the stage needs to know to
/// offer typing into it.
fn under_cursor(tree: &anyview_text::Tree, stage: &Stage) -> (NodeUnder, KeyUnder) {
    let Stage::Tree(shown) = stage else {
        return (NodeUnder::Nothing, KeyUnder::Absent);
    };
    let Some(row) = shown.row() else {
        return (NodeUnder::Nothing, KeyUnder::Absent);
    };
    let found = tree
        .visible(shown.open(), row.0..row.0.saturating_add(1))
        .into_iter()
        .next();
    match found {
        Some(visible) => {
            let under = match visible.row.kind {
                NodeKind::String
                | NodeKind::Number
                | NodeKind::True
                | NodeKind::False
                | NodeKind::Null => NodeUnder::Scalar,
                NodeKind::Object | NodeKind::Array | NodeKind::Unreadable => NodeUnder::Container,
            };
            let key = match visible.row.label {
                RowLabel::Key(_) => KeyUnder::Present,
                RowLabel::Root | RowLabel::Index(_) => KeyUnder::Absent,
            };
            (under, key)
        }
        None => (NodeUnder::Nothing, KeyUnder::Absent),
    }
}
