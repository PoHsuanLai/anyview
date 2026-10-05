//! JSON and JSON Lines: the tree stage's view. A file is parsed whole on a worker (`doc`); the
//! rows on screen are the visible nodes in a `VirtualList`, read a window at a time.

mod doc;
mod view;

pub use doc::TreeDoc;

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{OpenError, OpenLink};
use crate::{Command, PanelTab, PanelTabs, Stage, StageCommand, StageFamily, StageParams, Ticket};
use anyview_core::{Facts, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
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
        _link: &OpenLink,
    ) -> Result<TreeDoc, OpenError> {
        doc::open(src, sniffed)
    }

    fn facts(doc: &TreeDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(_doc: &TreeDoc) -> PanelTabs {
        PanelTabs::of(&[PanelTab::Info])
    }

    fn params(_doc: &TreeDoc, _stage: &Stage, _area: Option<Area>) -> StageParams {
        StageParams::default()
    }

    fn stage(doc: &Arc<TreeDoc>, cx: &StageCx) -> Element {
        rsx! { view::TreeContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(_doc: &TreeDoc, _cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
        vec![CapsuleSlot::button(
            Command::Stage(StageCommand::CollapseAll),
            "Collapse all",
            Icon::ChevronUp,
        )]
    }

    fn panel(_doc: &Arc<TreeDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }
}
