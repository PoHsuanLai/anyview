//! What a row of the tree view shows about one node.

use super::node::{Node, ScalarKind};

/// The most characters of a string a row previews before an ellipsis.
const PREVIEW_CHARS: usize = 80;

/// What a node is, for its icon and for deciding whether it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// An object: opens to its entries.
    Object,
    /// An array: opens to its items.
    Array,
    /// A string.
    String,
    /// A number.
    Number,
    /// `true`.
    True,
    /// `false`.
    False,
    /// `null`.
    Null,
}

/// What a row is called: its key in an object, its position in an array, or the document itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowLabel {
    /// The root of the document.
    Root,
    /// A key of an object.
    Key(String),
    /// A position in an array, zero-based.
    Index(u32),
}

/// How many children a node has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChildCount(pub u32);

/// One row of the tree view: enough to draw it without holding the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// What the row is called.
    pub label: RowLabel,
    /// What the node is.
    pub kind: NodeKind,
    /// A short rendering: a scalar's text (a string quoted and cut), or `{3 keys}` and `[2 items]`
    /// for a container.
    pub preview: String,
    /// How many rows it opens to; zero for a scalar.
    pub children: ChildCount,
}

fn count(len: usize) -> ChildCount {
    ChildCount(u32::try_from(len).unwrap_or(u32::MAX))
}

/// `{3 keys}`, `{1 key}` or `{}`.
fn object_preview(len: usize) -> String {
    match len {
        0 => "{}".to_owned(),
        1 => "{1 key}".to_owned(),
        n => format!("{{{n} keys}}"),
    }
}

/// `[2 items]`, `[1 item]` or `[]`.
fn array_preview(len: usize) -> String {
    match len {
        0 => "[]".to_owned(),
        1 => "[1 item]".to_owned(),
        n => format!("[{n} items]"),
    }
}

/// A string in quotes, cut to [`PREVIEW_CHARS`] characters with an ellipsis, line breaks shown as
/// `\n`.
fn string_preview(text: &str) -> String {
    let flat = text.replace('\n', "\\n").replace('\r', "\\r");
    let mut shown: String = flat.chars().take(PREVIEW_CHARS).collect();
    if flat.chars().count() > PREVIEW_CHARS {
        shown.push('…');
    }
    format!("\"{shown}\"")
}

/// The row for `node` under `label`.
pub(super) fn row(label: RowLabel, node: &Node) -> TreeRow {
    let (kind, preview, children) = match node {
        Node::Object(entries) => (
            NodeKind::Object,
            object_preview(entries.len()),
            count(entries.len()),
        ),
        Node::Array(items) => (
            NodeKind::Array,
            array_preview(items.len()),
            count(items.len()),
        ),
        Node::Scalar { kind, text } => {
            let (kind, preview) = match kind {
                ScalarKind::String => (NodeKind::String, string_preview(text)),
                ScalarKind::Number => (NodeKind::Number, text.clone()),
                ScalarKind::True => (NodeKind::True, text.clone()),
                ScalarKind::False => (NodeKind::False, text.clone()),
                ScalarKind::Null => (NodeKind::Null, text.clone()),
            };
            (kind, preview, ChildCount(0))
        }
    };
    TreeRow {
        label,
        kind,
        preview,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(kind: ScalarKind, text: &str) -> Node {
        Node::Scalar {
            kind,
            text: text.to_owned(),
        }
    }

    #[test]
    fn previews_summarise_containers_and_quote_and_cut_strings() {
        let long = "x".repeat(100);
        let cut = format!("\"{}…\"", "x".repeat(80));
        let item = || scalar(ScalarKind::Null, "null");
        // name, node, kind, preview, children
        let cases: Vec<(&str, Node, NodeKind, String, u32)> = vec![
            (
                "empty object",
                Node::Object(vec![]),
                NodeKind::Object,
                "{}".into(),
                0,
            ),
            (
                "one key",
                Node::Object(vec![("a".into(), item())]),
                NodeKind::Object,
                "{1 key}".into(),
                1,
            ),
            (
                "keys",
                Node::Object(vec![("a".into(), item()), ("b".into(), item())]),
                NodeKind::Object,
                "{2 keys}".into(),
                2,
            ),
            (
                "empty array",
                Node::Array(vec![]),
                NodeKind::Array,
                "[]".into(),
                0,
            ),
            (
                "one item",
                Node::Array(vec![item()]),
                NodeKind::Array,
                "[1 item]".into(),
                1,
            ),
            (
                "items",
                Node::Array(vec![item(), item(), item()]),
                NodeKind::Array,
                "[3 items]".into(),
                3,
            ),
            (
                "string",
                scalar(ScalarKind::String, "hi"),
                NodeKind::String,
                "\"hi\"".into(),
                0,
            ),
            (
                "long string is cut",
                scalar(ScalarKind::String, &long),
                NodeKind::String,
                cut,
                0,
            ),
            (
                "string with a line break",
                scalar(ScalarKind::String, "a\nb"),
                NodeKind::String,
                "\"a\\nb\"".into(),
                0,
            ),
            (
                "number",
                scalar(ScalarKind::Number, "1.5"),
                NodeKind::Number,
                "1.5".into(),
                0,
            ),
            (
                "true",
                scalar(ScalarKind::True, "true"),
                NodeKind::True,
                "true".into(),
                0,
            ),
            (
                "false",
                scalar(ScalarKind::False, "false"),
                NodeKind::False,
                "false".into(),
                0,
            ),
            (
                "null",
                scalar(ScalarKind::Null, "null"),
                NodeKind::Null,
                "null".into(),
                0,
            ),
        ];
        for (name, node, kind, preview, children) in cases {
            let got = row(RowLabel::Root, &node);
            assert_eq!(got.kind, kind, "{name} kind");
            assert_eq!(got.preview, preview, "{name} preview");
            assert_eq!(got.children, ChildCount(children), "{name} children");
        }
    }
}
