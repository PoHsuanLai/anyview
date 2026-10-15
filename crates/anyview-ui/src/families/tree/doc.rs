//! A JSON file indexed for showing: its tree, read a window at a time, and the rows of the Info tab.

use crate::io::OpenError;
use anyview_core::work::Stop;
use anyview_core::{
    Fact, FactGroup, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Sniffed, Source,
};
use anyview_fs::OnDisk;
use anyview_text::{Coverage, Tree};

/// The most bytes of a file the tree scans. The scan holds an index, not the file, so this is far
/// above what a person opens; a file longer than this shows its start.
pub const TREE_BYTES: anyview_core::ByteLen = anyview_core::ByteLen(1024 * 1024 * 1024);

/// An opened JSON or JSON Lines file.
#[derive(Debug)]
pub struct TreeDoc {
    /// The document, or for JSON Lines the array of its values.
    pub tree: Tree,
    /// Whether the tree reaches the end of the file.
    pub coverage: Coverage,
    /// The rows of the Info tab.
    pub facts: Facts,
}

/// Open the JSON file `src`. Blocking.
pub(super) fn open(src: &Source, sniffed: &Sniffed, stop: &Stop) -> Result<TreeDoc, OpenError> {
    let (FormatKind::Tree, FormatDetail::Tree(format)) = (sniffed.kind(), sniffed.detail()) else {
        return Err(OpenError::Unrecognised);
    };
    let (tree, coverage) = Tree::read_until(src.on_disk(), *format, TREE_BYTES, stop)?;
    let root = tree.row(&anyview_core::TreePath::root())?;
    let entries = match coverage {
        Coverage::Whole => root.children.0.to_string(),
        Coverage::Prefix => format!("{}+", root.children.0),
    };
    let facts = Facts::empty()
        .with(FactLabel::Kind, crate::families::kind_row(src, sniffed))
        .with(FactLabel::Size, FactValue::size(src.stamp().len))
        .with_fact(Fact::new(FactLabel::Items, FactValue::text(entries)).in_group(FactGroup::Data));
    Ok(TreeDoc {
        tree,
        coverage,
        facts,
    })
}
