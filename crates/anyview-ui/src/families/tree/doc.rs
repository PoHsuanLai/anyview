//! A JSON file indexed for showing: its tree, read a window at a time, and the rows of the Info tab.

use crate::io::OpenError;
use anyview_core::work::Stop;
use anyview_core::{
    Fact, FactGroup, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Sniffed, Source,
};
use anyview_fs::OnDisk;
use anyview_text::{Coverage, EDIT_BYTES, EditText, Tree, TreeSession};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The most bytes of a file the tree scans. The scan holds an index, not the file, so this is far
/// above what a person opens; a file longer than this shows its start.
pub const TREE_BYTES: anyview_core::ByteLen = anyview_core::ByteLen(1024 * 1024 * 1024);

/// An opened JSON or JSON Lines file.
#[derive(Debug)]
pub struct TreeDoc {
    /// The document as the file held it, or for JSON Lines the array of its values.
    pub tree: Tree,
    /// Whether the tree reaches the end of the file.
    pub coverage: Coverage,
    /// The rows of the Info tab.
    pub facts: Facts,
    /// The document held for editing, with the edits made to it, beside the document so the view
    /// that draws it and the save that writes it see the same ones. `None` for a file that cannot
    /// be written back (not UTF-8, too large, or only its start read).
    session: Mutex<Option<TreeSession>>,
}

impl TreeDoc {
    /// A document of `tree`, edited through `session` when there is one.
    pub(super) fn new(
        tree: Tree,
        coverage: Coverage,
        facts: Facts,
        session: Option<TreeSession>,
    ) -> Self {
        TreeDoc {
            tree,
            coverage,
            facts,
            session: Mutex::new(session),
        }
    }

    /// The edits, when the file can be edited.
    pub(crate) fn session(&self) -> MutexGuard<'_, Option<TreeSession>> {
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The document as it is now: with the edits made to it, when it is edited.
    pub(crate) fn now(&self) -> Tree {
        match self.session().as_ref() {
            Some(session) => session.tree().clone(),
            None => self.tree.clone(),
        }
    }

    /// Whether the nodes can be edited.
    pub(crate) fn is_editable(&self) -> bool {
        self.session().is_some()
    }
}

/// Open the JSON file `src`. Blocking.
pub(super) fn open(src: &Source, sniffed: &Sniffed, stop: &Stop) -> Result<TreeDoc, OpenError> {
    let (FormatKind::Tree, FormatDetail::Tree(format)) = (sniffed.kind(), sniffed.detail()) else {
        return Err(OpenError::Unrecognised);
    };
    let (tree, coverage, session) = match read_for_edit(src, *format) {
        Some(session) => (session.tree().clone(), Coverage::Whole, Some(session)),
        None => {
            let (tree, coverage) = Tree::read_until(src.on_disk(), *format, TREE_BYTES, stop)?;
            (tree, coverage, None)
        }
    };
    let root = tree.row(&anyview_core::TreePath::root())?;
    let entries = match coverage {
        Coverage::Whole => root.children.0.to_string(),
        Coverage::Prefix => format!("{}+", root.children.0),
    };
    let facts = Facts::empty()
        .with(FactLabel::Kind, crate::families::kind_row(src, sniffed))
        .with(FactLabel::Size, FactValue::size(src.stamp().len))
        .with_fact(Fact::new(FactLabel::Items, FactValue::text(entries)).in_group(FactGroup::Data));
    Ok(TreeDoc::new(tree, coverage, facts, session))
}

/// The file read once, whole, for editing: the tree is the session's, so the index and the edits
/// come from the same bytes. `None` when the file is too large to edit, is not UTF-8 or is not a
/// document, and the caller then scans it for showing only.
fn read_for_edit(src: &Source, format: anyview_core::TreeFormat) -> Option<TreeSession> {
    if src.stamp().len.0 > EDIT_BYTES {
        return None;
    }
    let text = EditText::read(src.path().as_path()).ok()?;
    TreeSession::open(text, format).ok()
}
