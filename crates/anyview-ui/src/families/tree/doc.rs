//! A JSON file parsed for showing: the whole tree, and the rows of the Info tab.

use crate::io::OpenError;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Sniffed, Source, TreeFormat,
};
use anyview_fs::OnDisk;
use anyview_text::{Coverage, Tree};

/// The most bytes of a file the tree reads. A JSON document is one value and is refused above
/// this; a JSON Lines file is read as far as it reaches.
pub const TREE_BYTES: anyview_core::ByteLen = anyview_core::ByteLen(64 * 1024 * 1024);

/// An opened JSON or JSON Lines file.
#[derive(Debug)]
pub struct TreeDoc {
    /// The document, or for JSON Lines the array of its lines' values.
    pub tree: Tree,
    /// Whether every line of a JSON Lines file is in the tree.
    pub coverage: Coverage,
    /// The rows of the Info tab.
    pub facts: Facts,
}

/// Open the JSON file `src`. Blocking.
pub(super) fn open(src: &Source, sniffed: &Sniffed) -> Result<TreeDoc, OpenError> {
    let (FormatKind::Tree, FormatDetail::Tree(format)) = (sniffed.kind(), sniffed.detail()) else {
        return Err(OpenError::Unrecognised);
    };
    let (tree, coverage) = Tree::read(src.on_disk(), *format, TREE_BYTES)?;
    let root = tree.row(&anyview_core::TreePath::root())?;
    let kind = match format {
        TreeFormat::Json => "JSON",
        TreeFormat::JsonLines => "JSON Lines",
    };
    let entries = match coverage {
        Coverage::Whole => root.children.0.to_string(),
        Coverage::Prefix => format!("{}+", root.children.0),
    };
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(kind))
        .with(FactLabel::Size, FactValue::size(src.stamp().len))
        .with(FactLabel::Entries, FactValue::text(entries));
    Ok(TreeDoc {
        tree,
        coverage,
        facts,
    })
}
