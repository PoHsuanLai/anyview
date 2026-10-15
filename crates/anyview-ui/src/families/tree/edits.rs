//! The tree's edits as the window carries them out: what the stage asks of the document (a value
//! typed in, a key renamed, a node added or taken away, an edit taken back) is done to the
//! document's edits, and the stage is told what they stand at and where its cursor goes. A save
//! hands the document's bytes over for the host to write. Text that does not fit the node it was
//! typed into (a word for a number) changes nothing.

use super::doc::TreeDoc;
use crate::families::view::{EditDone, SaveBytes};
use crate::{
    Changes, EditDepth, EditLog, EditRevision, EditSessionIn, EditSessionOut, Rewind, RowNo,
    RowOrder, Stage, StageIn, StageOut, TreeIn, TreeOut,
};
use anyview_core::{OpenNodes, TreePath};
use anyview_text::{Tree, TreeSession, Undid};

/// What the stage is told of the edits `session` holds.
pub(super) fn log_of(session: &TreeSession) -> EditLog {
    let count = |held: usize| EditDepth(u32::try_from(held).unwrap_or(u32::MAX));
    EditLog {
        changes: if session.is_modified() {
            Changes::Unsaved
        } else {
            Changes::Saved
        },
        undo: count(session.undoable()),
        redo: count(session.redoable()),
        rows: RowOrder::AsFile,
        revision: EditRevision(session.stamp().number()),
    }
}

fn logged(session: &TreeSession) -> StageIn {
    StageIn::Tree(TreeIn::Session(EditSessionIn::Logged(log_of(session))))
}

/// The path of the node at visible row `row`.
fn path_at(tree: &Tree, open: &OpenNodes, row: RowNo) -> Option<TreePath> {
    tree.visible(open, row.0..row.0.saturating_add(1))
        .into_iter()
        .next()
        .map(|visible| visible.path)
}

/// The stage told where its nodes and cursor are once the rows came out as `open` and `row`.
fn reshaped(tree: &Tree, open: OpenNodes, row: Option<RowNo>) -> StageIn {
    let last = tree.visible_count(&open).saturating_sub(1);
    let row = row.map(|row| RowNo(row.0.min(last)));
    StageIn::Tree(TreeIn::Reshaped { open, row })
}

/// Carry out what the stage asked of the edits.
pub(super) fn carry(doc: &TreeDoc, stage: &Stage, out: &StageOut) -> EditDone {
    let (Stage::Tree(shown), StageOut::Tree(out)) = (stage, out) else {
        return EditDone::default();
    };
    let mut held = doc.session();
    let Some(session) = held.as_mut() else {
        return EditDone::default();
    };
    let open = shown.open().clone();
    let tree = session.tree().clone();
    match out {
        TreeOut::Set { row, text } => {
            if let Some(path) = path_at(&tree, &open, *row) {
                // Text that does not fit the node changes nothing.
                let _refused = session.set_value(&path, text.as_str());
            }
            told(vec![logged(session)])
        }
        TreeOut::Rename { row, text } => {
            if let Some(path) = path_at(&tree, &open, *row) {
                let _refused = session.rename_key(&path, text.as_str());
            }
            told(vec![logged(session)])
        }
        TreeOut::Add(row) => {
            let added =
                path_at(&tree, &open, *row).and_then(|path| session.insert_below(&path).ok());
            let Some(added) = added else {
                return EditDone::default();
            };
            let (index, parent) = added.split_last().unwrap_or((0, TreePath::root()));
            let open = open.after_insert(&parent, index).opened(parent);
            let now = session.tree().clone();
            let row = now.visible_index(&open, &added).map(RowNo);
            told(vec![logged(session), reshaped(&now, open, row)])
        }
        TreeOut::Delete(row) => {
            let Some(path) = path_at(&tree, &open, *row) else {
                return EditDone::default();
            };
            if session.delete(&path).is_err() {
                return EditDone::default();
            }
            let (index, parent) = path.split_last().unwrap_or((0, TreePath::root()));
            let open = open.after_removal(&parent, index);
            let now = session.tree().clone();
            told(vec![logged(session), reshaped(&now, open, Some(*row))])
        }
        TreeOut::Session(EditSessionOut::Rewind(way)) => {
            let done = match way {
                Rewind::Undo => session.undo(),
                Rewind::Redo => session.redo(),
            };
            match done {
                Undid::Something => {
                    let now = session.tree().clone();
                    told(vec![logged(session), reshaped(&now, open, shown.row())])
                }
                Undid::Nothing => EditDone::default(),
            }
        }
        TreeOut::Session(EditSessionOut::Save) => EditDone {
            tell: Vec::new(),
            write: SaveBytes::Bytes(session.file_bytes(), session.stamp()),
        },
        TreeOut::Session(EditSessionOut::Discard) => {
            // Back to the state the file holds: taken back as far as the last save, or done again
            // up to it if the person had gone back past it.
            while session.is_modified() && session.undo() == Undid::Something {}
            while session.is_modified() && session.redo() == Undid::Something {}
            let now = session.tree().clone();
            told(vec![logged(session), reshaped(&now, open, shown.row())])
        }
        TreeOut::Find(_) => EditDone::default(),
    }
}

fn told(tell: Vec<StageIn>) -> EditDone {
    EditDone {
        tell,
        write: SaveBytes::Nothing,
    }
}
