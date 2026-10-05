//! The peek of JSON and JSON Lines: the top level of the tree.

use super::head::{PEEK_LINES, expect_kind, read_head};
use super::tally::{Tally, grouped};
use crate::encoding::{Coverage, TextCodec};
use crate::error::TextError;
use crate::tree::{Tree, TreeRow};
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, Sniffed, Source,
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
    /// For JSON Lines, how many values the file has as far as the budget let the peek see; for JSON
    /// the top-level count is exact in `root`.
    pub values: Option<Tally>,
    /// Which format it is.
    pub format: TreeFormat,
    /// How the bytes were decoded.
    pub encoding: TextCodec,
}

/// The peek of the kind `Tree`. A JSON document larger than the budget is not parsed: a document
/// cannot be read in part, so the peek is refused with [`TextError::JsonOverBudget`] and the pane
/// shows the file's own facts. JSON Lines is read line by line, so its start is always enough.
#[derive(Debug, Clone, Copy)]
pub struct TreePeek;

impl Peek for TreePeek {
    const KIND: FormatKind = FormatKind::Tree;
    type Peeked = TreePeeked;
    type Error = TextError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<TreePeeked, TextError> {
        expect_kind(sniffed, Self::KIND)?;
        let FormatDetail::Tree(format) = sniffed.detail() else {
            return Err(TextError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        let head = read_head(src, budget)?;
        if *format == TreeFormat::Json && head.coverage == Coverage::Prefix {
            return Err(TextError::JsonOverBudget);
        }
        let tree = Tree::parse(&head.text, *format)?;
        let root = tree.row(&TreePath::root())?;
        let values = match format {
            TreeFormat::JsonLines => Some(Tally::of(root.children.0 as usize, head.coverage)), // a u32 fits a usize
            TreeFormat::Json => None,
        };
        Ok(TreePeeked {
            top: tree.children(&TreePath::root(), 0..PEEK_LINES as u32)?, // 40
            keys: tree.top_level_keys(KEYS_NAMED),
            root,
            values,
            format: *format,
            encoding: head.codec,
        })
    }

    fn facts(peeked: &TreePeeked) -> Facts {
        let kind = match peeked.format {
            TreeFormat::Json => "JSON",
            TreeFormat::JsonLines => "JSON Lines",
        };
        let facts = Facts::empty().with(FactLabel::Kind, FactValue::text(kind));
        let facts = match (&peeked.values, peeked.keys.as_slice()) {
            (Some(values), _) => facts.with(FactLabel::Entries, FactValue::text(values.text())),
            (None, []) => facts.with(
                FactLabel::Entries,
                FactValue::text(grouped(peeked.root.children.0)),
            ),
            (None, keys) => facts.with(FactLabel::Keys, FactValue::text(keys_text(keys, peeked))),
        };
        facts.with(
            FactLabel::Encoding,
            FactValue::text(peeked.encoding.label()),
        )
    }
}

/// `name, version, deps and 3 more`: the keys named, then how many more the object has.
fn keys_text(keys: &[String], peeked: &TreePeeked) -> String {
    let named = keys.join(", ");
    let more = (peeked.root.children.0 as usize).saturating_sub(keys.len()); // a u32 fits a usize
    match more {
        0 => named,
        more => format!("{named} and {more} more"),
    }
}
