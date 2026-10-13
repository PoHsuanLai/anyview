//! What a row of the tree view shows about one node.

use super::grammar::Shape;

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
    /// A line of a JSON Lines file that is not a value; its preview is the line as written.
    Unreadable,
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

/// `{3 keys}`, `{1 key}` or `{}`.
fn object_preview(len: u32) -> String {
    match len {
        0 => "{}".to_owned(),
        1 => "{1 key}".to_owned(),
        n => format!("{{{n} keys}}"),
    }
}

/// `[2 items]`, `[1 item]` or `[]`.
fn array_preview(len: u32) -> String {
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

/// An unreadable line as written, cut to [`PREVIEW_CHARS`] characters, headed by what it is.
fn unreadable_preview(text: &str) -> String {
    let mut shown: String = text.chars().take(PREVIEW_CHARS).collect();
    if text.chars().count() > PREVIEW_CHARS {
        shown.push('…');
    }
    format!("unreadable: {shown}")
}

/// The row for a node of `shape` under `label`. `text` is what a scalar shows: a string's
/// characters, or a number as it was written; `len` is how many children a container has.
pub(super) fn row(label: RowLabel, shape: Shape, text: &str, len: u32) -> TreeRow {
    let (kind, preview, children) = match shape {
        Shape::Object => (NodeKind::Object, object_preview(len), ChildCount(len)),
        Shape::Array => (NodeKind::Array, array_preview(len), ChildCount(len)),
        Shape::String => (NodeKind::String, string_preview(text), ChildCount(0)),
        Shape::Number => (NodeKind::Number, text.to_owned(), ChildCount(0)),
        Shape::True => (NodeKind::True, "true".to_owned(), ChildCount(0)),
        Shape::False => (NodeKind::False, "false".to_owned(), ChildCount(0)),
        Shape::Null => (NodeKind::Null, "null".to_owned(), ChildCount(0)),
        Shape::Unreadable => (
            NodeKind::Unreadable,
            unreadable_preview(text),
            ChildCount(0),
        ),
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

    #[test]
    fn previews_summarise_containers_and_quote_and_cut_strings() {
        let long = "x".repeat(100);
        let cut = format!("\"{}…\"", "x".repeat(80));
        // name, shape, text, container length, kind, preview, children
        let cases: Vec<(&str, Shape, &str, u32, NodeKind, String, u32)> = vec![
            (
                "empty object",
                Shape::Object,
                "",
                0,
                NodeKind::Object,
                "{}".into(),
                0,
            ),
            (
                "one key",
                Shape::Object,
                "",
                1,
                NodeKind::Object,
                "{1 key}".into(),
                1,
            ),
            (
                "keys",
                Shape::Object,
                "",
                2,
                NodeKind::Object,
                "{2 keys}".into(),
                2,
            ),
            (
                "empty array",
                Shape::Array,
                "",
                0,
                NodeKind::Array,
                "[]".into(),
                0,
            ),
            (
                "one item",
                Shape::Array,
                "",
                1,
                NodeKind::Array,
                "[1 item]".into(),
                1,
            ),
            (
                "items",
                Shape::Array,
                "",
                3,
                NodeKind::Array,
                "[3 items]".into(),
                3,
            ),
            (
                "string",
                Shape::String,
                "hi",
                0,
                NodeKind::String,
                "\"hi\"".into(),
                0,
            ),
            (
                "long string is cut",
                Shape::String,
                &long,
                0,
                NodeKind::String,
                cut,
                0,
            ),
            (
                "string with a line break",
                Shape::String,
                "a\nb",
                0,
                NodeKind::String,
                "\"a\\nb\"".into(),
                0,
            ),
            (
                "number as written",
                Shape::Number,
                "1.50",
                0,
                NodeKind::Number,
                "1.50".into(),
                0,
            ),
            ("true", Shape::True, "", 0, NodeKind::True, "true".into(), 0),
            (
                "false",
                Shape::False,
                "",
                0,
                NodeKind::False,
                "false".into(),
                0,
            ),
            ("null", Shape::Null, "", 0, NodeKind::Null, "null".into(), 0),
        ];
        for (name, shape, text, len, kind, preview, children) in cases {
            let got = row(RowLabel::Root, shape, text, len);
            assert_eq!(got.kind, kind, "{name} kind");
            assert_eq!(got.preview, preview, "{name} preview");
            assert_eq!(got.children, ChildCount(children), "{name} children");
        }
    }
}
