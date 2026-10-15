//! The tree stage's edits: typing into the value or the key of the node under the cursor, adding
//! a node below it and taking it away. What a change does to the file is the window's to carry out:
//! the stage names the row (a place among the visible rows) and hears back how the rows came out.

use super::model::{KeyUnder, NodeUnder, TreeIn, TreeOut, TreeParams};
use crate::stage::data_edits::{self, AfterTyping, DataEdits, Taken, TypingIn, TypingState};
use crate::stage::row::RowNo;
use crate::stage::text::Editable;
use anyview_core::OpenNodes;

/// What an input about edits left: the open nodes, the cursor, the edits, and what is asked.
pub(super) struct Worked {
    pub(super) open: OpenNodes,
    pub(super) row: Option<RowNo>,
    pub(super) edits: DataEdits,
    pub(super) outs: Vec<TreeOut>,
}

/// The nodes, cursor and edits after `input`, which is one of the inputs about edits.
pub(super) fn apply(
    (open, row, edits): (OpenNodes, Option<RowNo>, DataEdits),
    input: TreeIn,
    params: &TreeParams,
) -> Worked {
    let still = |open: OpenNodes, row: Option<RowNo>, edits: DataEdits| Worked {
        open,
        row,
        edits,
        outs: Vec::new(),
    };
    let editable = params.editable;
    match (input, row, editable) {
        (TreeIn::Typing(TypingIn::Begin(seed)), Some(_), Editable::Yes)
            if params.under == NodeUnder::Scalar =>
        {
            let (edits, _) = data_edits::typing(edits, TypingIn::Begin(seed), editable);
            still(open, row, edits)
        }
        (TreeIn::Typing(TypingIn::Begin(_)), _, _) => still(open, row, edits),
        (TreeIn::Typing(input), _, _) => {
            let (edits, taken) = data_edits::typing(edits, input, editable);
            let mut moved = row;
            let mut outs = Vec::new();
            match (taken, row) {
                (Taken::Value(text, then), Some(at)) => {
                    outs.push(TreeOut::Set { row: at, text });
                    moved = after(at, then, params);
                }
                (Taken::Key(text, then), Some(at)) => {
                    outs.push(TreeOut::Rename { row: at, text });
                    moved = after(at, then, params);
                }
                (Taken::Value(..) | Taken::Key(..) | Taken::Nothing, _) => {}
            }
            Worked {
                open,
                row: moved,
                edits,
                outs,
            }
        }
        (TreeIn::RenameKey, Some(_), Editable::Yes)
            if params.key == KeyUnder::Present && edits.typing == TypingState::Idle =>
        {
            let edits = DataEdits {
                typing: TypingState::Key,
                ..edits
            };
            still(open, row, edits)
        }
        (TreeIn::AddItem, Some(at), Editable::Yes) => Worked {
            open,
            row,
            edits,
            outs: vec![TreeOut::Add(at)],
        },
        // The root is the document itself: it is not taken away.
        (TreeIn::DeleteItem, Some(at), Editable::Yes) if at != RowNo(0) => Worked {
            open,
            row,
            edits,
            outs: vec![TreeOut::Delete(at)],
        },
        (TreeIn::Reshaped { open, row }, _, _) => still(open, row, edits),
        (TreeIn::Session(input), _, _) => {
            let (edits, out) = data_edits::session(edits, input);
            Worked {
                open,
                row,
                edits,
                outs: out.map(TreeOut::Session).into_iter().collect(),
            }
        }
        (
            TreeIn::RenameKey | TreeIn::AddItem | TreeIn::DeleteItem,
            _,
            Editable::Yes | Editable::No,
        ) => still(open, row, edits),
        // The inputs about the open nodes, the cursor and the find are not about edits.
        (
            TreeIn::Toggle(_)
            | TreeIn::Open(_)
            | TreeIn::Close(_)
            | TreeIn::CollapseAll
            | TreeIn::Select(_)
            | TreeIn::Move(_)
            | TreeIn::Deselect
            | TreeIn::Show { .. }
            | TreeIn::Find(_)
            | TreeIn::Results { .. }
            | TreeIn::NextHit
            | TreeIn::PreviousHit
            | TreeIn::GoToHit(_)
            | TreeIn::CloseFind
            | TreeIn::ZoomStep(_)
            | TreeIn::ZoomActual
            | TreeIn::Elapsed,
            _,
            _,
        ) => still(open, row, edits),
    }
}

/// The row the cursor is on once typing at `at` is taken: the next one down for Return, else the
/// same.
fn after(at: RowNo, then: AfterTyping, params: &TreeParams) -> Option<RowNo> {
    match then {
        AfterTyping::Down if at.0.saturating_add(1) < params.rows => Some(RowNo(at.0 + 1)),
        AfterTyping::Down | AfterTyping::Right | AfterTyping::Stay => Some(at),
    }
}
