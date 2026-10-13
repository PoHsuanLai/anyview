//! The peek of JSON and JSON Lines: the top level of the tree.

use super::head::{PEEK_LINES, expect_kind};
use super::tally::{Tally, grouped};
use crate::encoding::{Coverage, TextCodec};
use crate::error::TextError;
use crate::tree::{Tree, TreeRow};
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Input, Peek, PeekBudget, Sniffed,
    TreeFormat, TreePath,
};

/// How many top-level keys the facts name before "and N more".
const KEYS_NAMED: usize = 8;

/// What a peek of a JSON file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreePeeked {
    /// The document, or for JSON Lines the array of its values.
    pub root: TreeRow,
    /// The first rows of the top level.
    pub top: Vec<TreeRow>,
    /// The keys of the top-level object, in file order, at most [`KEYS_NAMED`] of them.
    pub keys: Vec<String>,
    /// How many top-level values the file has as far as the budget let the peek see; `None` for a
    /// JSON document it read whole, whose top-level count is exact in `root`.
    pub values: Option<Tally>,
    /// Which format it is.
    pub format: TreeFormat,
    /// How the bytes were decoded.
    pub encoding: TextCodec,
}

/// The peek of the kind `Tree`. The file is scanned as far as the budget reaches, never parsed
/// whole: a JSON document larger than the budget shows the top-level keys and items that fit, and
/// counts them as a lower bound. Only a document that is one value larger than the budget (a
/// single enormous string) is refused with [`TextError::JsonOverBudget`].
#[derive(Debug, Clone, Copy)]
pub struct TreePeek;

impl Peek for TreePeek {
    const KIND: FormatKind = FormatKind::Tree;
    type Peeked = TreePeeked;
    type Error = TextError;

    fn peek(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<TreePeeked, TextError> {
        expect_kind(sniffed, Self::KIND)?;
        let FormatDetail::Tree(format) = sniffed.detail() else {
            return Err(TextError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        let (tree, coverage) = Tree::read(src, *format, budget.bytes)?;
        let root = tree.row(&TreePath::root())?;
        let count = root.children;
        let values = match (format, coverage) {
            (TreeFormat::Json, Coverage::Whole) => None,
            (TreeFormat::Json | TreeFormat::JsonLines, _) => {
                Some(Tally::of(count.0 as usize, coverage)) // a u32 fits a usize
            }
        };
        Ok(TreePeeked {
            top: tree.children(&TreePath::root(), 0..PEEK_LINES as u32)?, // 40
            keys: tree.top_level_keys(KEYS_NAMED),
            root,
            values,
            format: *format,
            encoding: tree.encoding(),
        })
    }

    fn facts(peeked: &TreePeeked) -> Facts {
        let kind = match peeked.format {
            TreeFormat::Json => "JSON",
            TreeFormat::JsonLines => "JSON Lines",
        };
        let facts = Facts::empty().with(FactLabel::Kind, FactValue::text(kind));
        let facts = match (&peeked.values, peeked.keys.as_slice()) {
            (Some(values), []) => facts.with(FactLabel::Entries, FactValue::text(values.text())),
            (None, []) => facts.with(
                FactLabel::Entries,
                FactValue::text(grouped(peeked.root.children.0)),
            ),
            (values, keys) => facts.with(
                FactLabel::Keys,
                FactValue::text(keys_text(keys, peeked.root.children.0, values.as_ref())),
            ),
        };
        facts.with(
            FactLabel::Encoding,
            FactValue::text(peeked.encoding.label()),
        )
    }
}

/// `name, version, deps and 3 more`: the keys named, then how many more the object has, with a
/// plus when the peek saw only the start of it.
fn keys_text(keys: &[String], total: u32, seen: Option<&Tally>) -> String {
    let named = keys.join(", ");
    let more = (total as usize).saturating_sub(keys.len()); // a u32 fits a usize
    match (more, seen) {
        (0, Some(Tally::AtLeast(_))) => format!("{named} and more"),
        (0, _) => named,
        (more, Some(Tally::AtLeast(_))) => format!("{named} and {more}+ more"),
        (more, _) => format!("{named} and {more} more"),
    }
}
