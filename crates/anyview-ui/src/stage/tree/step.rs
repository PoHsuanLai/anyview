//! The tree stage's transitions.

use super::model::{TreeIn, TreeOut, TreeParams, TreeStage};
use crate::stage::row::RowNo;
use anyview_core::OpenNodes;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (TreeStage, Vec<TreeOut>);

impl Machine for TreeStage {
    type In = TreeIn;
    type Out = TreeOut;
    type Params = TreeParams;
    type Ctx = ();

    fn step(self, input: TreeIn, _at: Stamp, _params: &TreeParams, _cx: &()) -> Step {
        match self {
            TreeStage::Browsing { open } => browsing(open, input),
            TreeStage::Selected { open, row } => selected(open, row, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TreeStage::Browsing { open: _ } | TreeStage::Selected { open: _, row: _ } => None,
        }
    }
}

/// The open set `input` leads to, or `None` when `input` does not change it.
fn opened(open: &OpenNodes, input: &TreeIn) -> Option<OpenNodes> {
    match input {
        TreeIn::Toggle(path) => Some(open.clone().toggled(path)),
        TreeIn::Open(path) => Some(open.clone().opened(path.clone())),
        TreeIn::Close(path) => Some(open.clone().closed(path)),
        TreeIn::CollapseAll => Some(OpenNodes::top_level()),
        TreeIn::Select(_) | TreeIn::Deselect | TreeIn::Elapsed => None,
    }
}

fn browsing(open: OpenNodes, input: TreeIn) -> Step {
    if let Some(open) = opened(&open, &input) {
        return (TreeStage::Browsing { open }, vec![]);
    }
    match input {
        TreeIn::Select(row) => (TreeStage::Selected { open, row }, vec![]),
        TreeIn::Toggle(_)
        | TreeIn::Open(_)
        | TreeIn::Close(_)
        | TreeIn::CollapseAll
        | TreeIn::Deselect
        | TreeIn::Elapsed => (TreeStage::Browsing { open }, vec![]),
    }
}

fn selected(open: OpenNodes, row: RowNo, input: TreeIn) -> Step {
    if let Some(next) = opened(&open, &input) {
        // Collapsing everything may take the cursor's row away: it goes back to the top.
        return match input {
            TreeIn::CollapseAll => (TreeStage::Browsing { open: next }, vec![]),
            TreeIn::Toggle(_) | TreeIn::Open(_) | TreeIn::Close(_) => {
                (TreeStage::Selected { open: next, row }, vec![])
            }
            TreeIn::Select(_) | TreeIn::Deselect | TreeIn::Elapsed => {
                (TreeStage::Selected { open, row }, vec![])
            }
        };
    }
    match input {
        TreeIn::Select(row) => (TreeStage::Selected { open, row }, vec![]),
        TreeIn::Deselect => (TreeStage::Browsing { open }, vec![]),
        TreeIn::Toggle(_)
        | TreeIn::Open(_)
        | TreeIn::Close(_)
        | TreeIn::CollapseAll
        | TreeIn::Elapsed => (TreeStage::Selected { open, row }, vec![]),
    }
}
