//! JSON and JSON Lines as a tree, opened one level at a time.
//!
//! The whole document is parsed (keys in file order), and the view asks for the children of one
//! node at a time, a window of them, so an array of a million items costs one screenful of rows.

mod node;
mod rows;

#[cfg(test)]
mod tests;

pub use rows::{ChildCount, NodeKind, RowLabel, TreeRow};

use crate::encoding::{Coverage, TextCodec, detect};
use crate::error::TextError;
use anyview_core::{LineIndex, TreeFormat};
use node::Node;
use std::ops::Range;

/// Where a node is: the position of each child to take, from the root. The root is the empty path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TreePath(Vec<u32>);

impl TreePath {
    /// The root.
    pub fn root() -> Self {
        TreePath(Vec::new())
    }

    /// The path to this node's child at `position`.
    pub fn child(&self, position: u32) -> Self {
        let mut steps = self.0.clone();
        steps.push(position);
        TreePath(steps)
    }
}

/// A parsed JSON document, or the values of a JSON Lines file as the items of an array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    format: TreeFormat,
    root: Node,
}

fn json_error(error: &serde_json::Error, line_offset: u32) -> TextError {
    TextError::Json {
        line: LineIndex(line_offset + u32::try_from(error.line().saturating_sub(1)).unwrap_or(0)),
        column: u32::try_from(error.column()).unwrap_or(u32::MAX),
        reason: error.to_string(),
    }
}

impl Tree {
    /// The document in `text`: one JSON value for `Json`, a value per non-blank line for
    /// `JsonLines`. A malformed one names its line and column.
    pub fn parse(text: &str, format: TreeFormat) -> Result<Self, TextError> {
        let root = match format {
            TreeFormat::Json => {
                serde_json::from_str::<Node>(text).map_err(|e| json_error(&e, 0))?
            }
            TreeFormat::JsonLines => {
                let mut items = Vec::new();
                for (number, line) in text.lines().enumerate() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let line_number = u32::try_from(number).unwrap_or(u32::MAX);
                    let item = serde_json::from_str::<Node>(line)
                        .map_err(|e| json_error(&e, line_number))?;
                    items.push(item);
                }
                Node::Array(items)
            }
        };
        Ok(Tree { format, root })
    }

    /// The document in a file's bytes: the encoding is detected, a byte-order mark skipped.
    pub fn parse_bytes(bytes: &[u8], format: TreeFormat) -> Result<Self, TextError> {
        let detected = detect(bytes, Coverage::Whole);
        let body = bytes.get(usize::from(detected.mark)..).unwrap_or_default();
        Tree::parse(&TextCodec::decode(detected.codec, body), format)
    }

    /// Which format it was read as.
    pub fn format(&self) -> TreeFormat {
        self.format
    }

    fn node(&self, path: &TreePath) -> Option<&Node> {
        path.0.iter().try_fold(&self.root, |node, step| {
            let step = *step as usize; // a u32 fits a usize
            match node {
                Node::Object(entries) => entries.get(step).map(|(_, child)| child),
                Node::Array(items) => items.get(step),
                Node::Scalar { .. } => None,
            }
        })
    }

    /// The row for the node at `path`, labelled as the root when the path is empty and otherwise
    /// by the last step's key or index.
    pub fn row(&self, path: &TreePath) -> Result<TreeRow, TextError> {
        let node = self.node(path).ok_or(TextError::NoSuchNode)?;
        let Some((last, parent)) = path.0.split_last() else {
            return Ok(rows::row(RowLabel::Root, node));
        };
        let parent_node = self
            .node(&TreePath(parent.to_vec()))
            .ok_or(TextError::NoSuchNode)?;
        let label = match parent_node {
            Node::Object(entries) => entries
                .get(*last as usize)
                .map(|(key, _)| RowLabel::Key(key.clone())),
            Node::Array(_) => Some(RowLabel::Index(*last)),
            Node::Scalar { .. } => None,
        }
        .ok_or(TextError::NoSuchNode)?;
        Ok(rows::row(label, node))
    }

    /// The rows for the children of the node at `path` in `window`, cut to the children that
    /// exist. A scalar has none.
    pub fn children(&self, path: &TreePath, window: Range<u32>) -> Result<Vec<TreeRow>, TextError> {
        let node = self.node(path).ok_or(TextError::NoSuchNode)?;
        let slice = |len: usize| {
            let end = (window.end as usize).min(len); // a u32 fits a usize
            (window.start as usize).min(end)..end
        };
        Ok(match node {
            Node::Object(entries) => entries[slice(entries.len())]
                .iter()
                .map(|(key, child)| rows::row(RowLabel::Key(key.clone()), child))
                .collect(),
            Node::Array(items) => {
                let range = slice(items.len());
                let first = u32::try_from(range.start).unwrap_or(u32::MAX);
                items[range]
                    .iter()
                    .zip(first..)
                    .map(|(child, index)| rows::row(RowLabel::Index(index), child))
                    .collect()
            }
            Node::Scalar { .. } => Vec::new(),
        })
    }

    /// The keys of the root object in file order, at most `limit` of them; empty when the root is
    /// not an object.
    pub fn top_level_keys(&self, limit: usize) -> Vec<String> {
        match &self.root {
            Node::Object(entries) => entries
                .iter()
                .take(limit)
                .map(|(key, _)| key.clone())
                .collect(),
            Node::Array(_) | Node::Scalar { .. } => Vec::new(),
        }
    }
}
