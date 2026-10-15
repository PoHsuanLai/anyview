//! The tree stage's transitions.

use super::edit::{self, Worked};
use super::model::{Look, TreeIn, TreeOut, TreeParams, TreeStage};
use crate::stage::find::{FindHits, FindOut, HitStep, StageFind};
use crate::stage::row::RowNo;
use crate::stage::zoom::ContentScale;
use crate::typed::TypedText;
use anyview_core::OpenNodes;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (TreeStage, Vec<TreeOut>);

impl Machine for TreeStage {
    type In = TreeIn;
    type Out = TreeOut;
    type Params = TreeParams;
    type Ctx = ();

    fn step(self, input: TreeIn, _at: Stamp, params: &TreeParams, _cx: &()) -> Step {
        let (open, row, look) = self.into_parts();
        let (open, row, look, outs) = advance(open, row, look, input, params);
        (TreeStage::of(open, row, look), outs)
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            TreeStage::Browsing {
                open: _,
                find: _,
                scale: _,
                edits: _,
            }
            | TreeStage::Selected {
                open: _,
                row: _,
                find: _,
                scale: _,
                edits: _,
            } => None,
        }
    }
}

/// The outputs that show the current hit of `hits`, if there is one.
fn shown(hits: FindHits) -> Vec<TreeOut> {
    hits.current()
        .map(|hit| TreeOut::Find(FindOut::ShowHit(hit)))
        .into_iter()
        .collect()
}

/// The find after `moved` changed which hit is current, and the output that shows the hit it
/// lands on; with no find up, nothing moves.
fn hit_moved(
    find: Option<StageFind>,
    moved: impl FnOnce(FindHits) -> FindHits,
) -> (Option<StageFind>, Vec<TreeOut>) {
    match find {
        Some(StageFind { query, hits }) => {
            let hits = moved(hits);
            (Some(StageFind { query, hits }), shown(hits))
        }
        None => (None, vec![]),
    }
}

/// The find after the search answered `asked` with its hits: only the search for the text now up
/// is heard, and the reader is taken to the hit nearest where they are.
fn answered(
    find: Option<StageFind>,
    asked: &TypedText,
    hits: FindHits,
) -> (Option<StageFind>, Vec<TreeOut>) {
    match find {
        Some(StageFind { query, .. }) if query == *asked => {
            (Some(StageFind { query, hits }), shown(hits))
        }
        other => (other, vec![]),
    }
}

/// The open nodes, the cursor, the find and the zoom after `input`, and what it asks for.
fn advance(
    open: OpenNodes,
    row: Option<RowNo>,
    look: Look,
    input: TreeIn,
    params: &TreeParams,
) -> (OpenNodes, Option<RowNo>, Look, Vec<TreeOut>) {
    let Look { find, scale, edits } = look;
    let kept =
        |open: OpenNodes, row: Option<RowNo>, find: Option<StageFind>, outs: Vec<TreeOut>| {
            (
                open,
                row,
                Look {
                    find,
                    scale,
                    edits: edits.clone(),
                },
                outs,
            )
        };
    match input {
        TreeIn::Toggle(path) => kept(open.toggled(&path), row, find, vec![]),
        TreeIn::Open(path) => kept(open.opened(path), row, find, vec![]),
        TreeIn::Close(path) => kept(open.closed(&path), row, find, vec![]),
        // Collapsing everything may take the cursor's row away: it goes back to the top.
        TreeIn::CollapseAll => kept(OpenNodes::top_level(), None, find, vec![]),
        TreeIn::Select(picked) => kept(open, Some(picked), find, vec![]),
        TreeIn::Move(step) => {
            let moved = step.from(row, params.rows, params.page).or(row);
            kept(open, moved, find, vec![])
        }
        TreeIn::Deselect => kept(open, None, find, vec![]),
        TreeIn::Show { opened, row: at } => {
            let open = opened.into_iter().fold(open, OpenNodes::opened);
            kept(open, Some(at), find, vec![])
        }
        TreeIn::Find(query) => {
            let outs = vec![TreeOut::Find(FindOut::asked(&query))];
            kept(open, row, Some(StageFind::asked(query)), outs)
        }
        TreeIn::Results {
            query,
            count,
            nearest,
        } => {
            let (find, outs) = answered(find, &query, FindHits::answered(count, nearest));
            kept(open, row, find, outs)
        }
        TreeIn::NextHit => {
            let (find, outs) = hit_moved(find, |hits| hits.stepped(HitStep::Next));
            kept(open, row, find, outs)
        }
        TreeIn::PreviousHit => {
            let (find, outs) = hit_moved(find, |hits| hits.stepped(HitStep::Previous));
            kept(open, row, find, outs)
        }
        TreeIn::GoToHit(hit) => {
            let (find, outs) = hit_moved(find, |hits| hits.jumped(hit));
            kept(open, row, find, outs)
        }
        TreeIn::CloseFind => {
            let outs = match find {
                Some(_) => vec![TreeOut::Find(FindOut::Clear)],
                None => vec![],
            };
            kept(open, row, None, outs)
        }
        TreeIn::ZoomStep(dir) => (
            open,
            row,
            Look {
                find,
                scale: scale.stepped(dir),
                edits,
            },
            vec![],
        ),
        TreeIn::ZoomActual => (
            open,
            row,
            Look {
                find,
                scale: ContentScale::ACTUAL,
                edits,
            },
            vec![],
        ),
        TreeIn::Typing(_)
        | TreeIn::RenameKey
        | TreeIn::AddItem
        | TreeIn::DeleteItem
        | TreeIn::Reshaped { .. }
        | TreeIn::Session(_) => {
            let worked = edit::apply((open, row, edits), input, params);
            let Worked {
                open,
                row,
                edits,
                outs,
            } = worked;
            (open, row, Look { find, scale, edits }, outs)
        }
        TreeIn::Elapsed => kept(open, row, find, vec![]),
    }
}
