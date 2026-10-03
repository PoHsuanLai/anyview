//! A parsed JSON value that keeps what a viewer shows: keys in file order, duplicates included,
//! and each number as it was written.

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::fmt;

/// What a scalar is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ScalarKind {
    /// `null`.
    Null,
    /// `true`.
    True,
    /// `false`.
    False,
    /// A number.
    Number,
    /// A string.
    String,
}

/// One value of a JSON document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    /// A value with no children, as text: the string itself, or the number, `true`, `false` or
    /// `null` as written.
    Scalar { kind: ScalarKind, text: String },
    /// An array.
    Array(Vec<Node>),
    /// An object: its entries in file order.
    Object(Vec<(String, Node)>),
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Node, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

fn scalar(kind: ScalarKind, text: impl Into<String>) -> Node {
    Node::Scalar {
        kind,
        text: text.into(),
    }
}

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Node, E> {
        Ok(match value {
            true => scalar(ScalarKind::True, "true"),
            false => scalar(ScalarKind::False, "false"),
        })
    }

    fn visit_i64<E>(self, value: i64) -> Result<Node, E> {
        Ok(scalar(ScalarKind::Number, value.to_string()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Node, E> {
        Ok(scalar(ScalarKind::Number, value.to_string()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Node, E> {
        Ok(scalar(ScalarKind::Number, value.to_string()))
    }

    fn visit_str<E>(self, value: &str) -> Result<Node, E> {
        Ok(scalar(ScalarKind::String, value))
    }

    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(scalar(ScalarKind::Null, "null"))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Node>()? {
            items.push(item);
        }
        Ok(Node::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Node>()? {
            entries.push((key, value));
        }
        Ok(Node::Object(entries))
    }
}
