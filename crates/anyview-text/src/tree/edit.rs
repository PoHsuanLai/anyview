//! Editing a JSON document in place. The document is held as its own bytes, and an edit is a splice
//! of them found from the byte offsets the tree keeps: a changed value replaces the bytes of the
//! value, a deleted node takes its comma and the white space that led to it, and a node added
//! below another is written with the separator the document already uses between its nodes. Every
//! byte the person did not touch is the byte it was, so the indentation style of the file stays
//! the file's own and a diff shows only what changed.

use super::grammar::{Child, Form, Shape};
use super::{Located, Tree};
use crate::edit::{EditStamp, EditText, Log};
use crate::encoding::{Coverage, TextCodec};
use crate::error::TextError;
use crate::table::Undid;
use anyview_core::{Deadline, TextEncoding, TreeFormat, TreePath};
use std::ops::Range;
use std::sync::Arc;

/// Why an edit to the document was not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreeEditError {
    /// The text is not a number, and the node is one.
    #[error("that is not a number")]
    NotANumber,
    /// The text is neither `true` nor `false`, and the node is one of them.
    #[error("that is neither true nor false")]
    NotTrueOrFalse,
    /// The node cannot be changed this way (an object or an array has no text to type, a line of
    /// a JSON Lines file that is not a value has none either, and the root has no key).
    #[error("that cannot be changed this way")]
    NotEditable,
    /// The document could not be read, or the node is not in it.
    #[error(transparent)]
    Text(#[from] TextError),
}

/// One step of the history: `removed` at `at` was replaced by `added`.
#[derive(Debug, Clone)]
struct Splice {
    at: usize,
    removed: Vec<u8>,
    added: Vec<u8>,
}

/// A JSON document being edited: its bytes, the tree read from them, and the history.
#[derive(Debug)]
pub struct TreeSession {
    mark: Vec<u8>,
    bytes: Vec<u8>,
    format: TreeFormat,
    tree: Tree,
    log: Log<Splice>,
}

/// The tree of `bytes`, a whole document in UTF-8.
fn read(bytes: &[u8], format: TreeFormat) -> Result<Tree, TextError> {
    let utf8 = TextCodec::Unicode(TextEncoding::Utf8);
    let end = bytes.len() as u64; // a usize fits a u64
    Tree::open(
        Arc::new(bytes.to_vec()),
        (utf8, utf8),
        (0, end),
        format,
        Coverage::Whole,
        // The bytes are in memory already: there is nothing slow to stop.
        &Deadline::NEVER,
    )
}

/// A node, the children beside it and the container it is in.
struct Around {
    parent: Located,
    prev: Option<Child>,
    this: Child,
    next: Option<Child>,
}

impl Tree {
    /// The node at `path`, which is not the root, with its neighbours.
    fn around(&self, path: &TreePath) -> Result<Around, TextError> {
        let (index, parent_path) = path.split_last().ok_or(TextError::NoSuchNode)?;
        let parent = self.locate(&parent_path)?;
        let level = parent.level.clone().ok_or(TextError::NoSuchNode)?;
        let from = index.saturating_sub(1);
        let found = self.read_children(&parent_path, &level, from..index.saturating_add(2))?;
        let pick = |want: u32| {
            found
                .iter()
                .find(|(at, _)| *at == want)
                .map(|(_, child)| child.clone())
        };
        let this = pick(index).ok_or(TextError::NoSuchNode)?;
        let prev = index.checked_sub(1).and_then(&pick);
        let next = pick(index.saturating_add(1));
        Ok(Around {
            parent,
            prev,
            this,
            next,
        })
    }
}

/// Where a child starts: its key if it has one, else its value.
fn start_of(child: &Child) -> usize {
    let start = child
        .key
        .as_ref()
        .map_or(child.value.start, |key| key.start);
    start as usize // an offset in a document held in memory fits a usize
}

fn bounds(range: &Range<u64>) -> Range<usize> {
    range.start as usize..range.end as usize // an offset in a document held in memory fits a usize
}

/// The literal a node of `shape` takes for the text the person typed, so the type of the node is
/// kept: a string is quoted, a number must be one, `true` and `false` stay themselves, and `null`
/// becomes whatever the text is as a JSON scalar, else a string.
fn literal(shape: Shape, typed: &str) -> Result<String, TreeEditError> {
    let trimmed = typed.trim();
    match shape {
        Shape::String => serde_json::to_string(typed).map_err(|_| TreeEditError::NotEditable),
        Shape::Number => serde_json::from_str::<serde_json::Number>(trimmed)
            .map(|_| trimmed.to_owned())
            .map_err(|_| TreeEditError::NotANumber),
        Shape::True | Shape::False => match trimmed {
            "true" | "false" => Ok(trimmed.to_owned()),
            _ => Err(TreeEditError::NotTrueOrFalse),
        },
        Shape::Null => match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(
                serde_json::Value::Null
                | serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_),
            ) => Ok(trimmed.to_owned()),
            Ok(serde_json::Value::Array(_) | serde_json::Value::Object(_)) | Err(_) => {
                serde_json::to_string(typed).map_err(|_| TreeEditError::NotEditable)
            }
        },
        Shape::Object | Shape::Array | Shape::Unreadable => Err(TreeEditError::NotEditable),
    }
}

/// The key a node added to an object is given, until the person names it.
const NEW_KEY: &str = "key";

impl TreeSession {
    /// The document in `text`, read as `format`.
    pub fn open(text: EditText, format: TreeFormat) -> Result<Self, TextError> {
        let (bytes, bom) = text.into_parts();
        let tree = read(&bytes, format)?;
        Ok(TreeSession {
            mark: bom.mark().to_vec(),
            bytes,
            format,
            tree,
            log: Log::default(),
        })
    }

    /// The document as it is now.
    #[must_use]
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// The file as it would be written now: its byte-order mark, if it had one, and the document.
    #[must_use]
    pub fn file_bytes(&self) -> Vec<u8> {
        let mut out = self.mark.clone();
        out.extend_from_slice(&self.bytes);
        out
    }

    /// The text of the value at `path` as a person edits it: a string's characters, whole; any
    /// other scalar as it is written. `None` for an object, an array and a line that is no value.
    #[must_use]
    pub fn value_text(&self, path: &TreePath) -> Option<String> {
        let (span, shape) = self.value_of(path).ok()?;
        let raw = self.bytes.get(bounds(&span))?;
        match shape {
            Shape::String => serde_json::from_slice::<String>(raw).ok(),
            Shape::Number | Shape::True | Shape::False | Shape::Null => {
                Some(String::from_utf8_lossy(raw).into_owned())
            }
            Shape::Object | Shape::Array | Shape::Unreadable => None,
        }
    }

    /// The key of the node at `path`, whole. `None` for a node with none.
    #[must_use]
    pub fn key_text(&self, path: &TreePath) -> Option<String> {
        let around = self.tree.around(path).ok()?;
        let raw = self.bytes.get(bounds(&around.this.key?))?;
        serde_json::from_slice::<String>(raw).ok()
    }

    /// Where the value at `path` is, and what it is.
    fn value_of(&self, path: &TreePath) -> Result<(Range<u64>, Shape), TextError> {
        if path.split_last().is_some() {
            let around = self.tree.around(path)?;
            Ok((around.this.value, around.this.shape))
        } else {
            let root = self.tree.locate(path)?;
            Ok((root.span, root.shape))
        }
    }

    /// Replace `range` of the bytes with `added`, as a step of the history.
    fn splice(&mut self, range: Range<usize>, added: Vec<u8>) -> Result<(), TreeEditError> {
        let removed = self.bytes.get(range.clone()).unwrap_or_default().to_vec();
        let step = Splice {
            at: range.start,
            removed,
            added,
        };
        self.apply(&step)?;
        self.log.push(step);
        Ok(())
    }

    /// Make `step`; the document is unchanged if the result would not be one.
    fn apply(&mut self, step: &Splice) -> Result<(), TreeEditError> {
        let end = step.at + step.removed.len();
        let mut next = Vec::with_capacity(self.bytes.len() + step.added.len());
        next.extend_from_slice(self.bytes.get(..step.at).unwrap_or_default());
        next.extend_from_slice(&step.added);
        next.extend_from_slice(self.bytes.get(end..).unwrap_or_default());
        let tree = read(&next, self.format)?;
        self.bytes = next;
        self.tree = tree;
        Ok(())
    }

    /// Change the value at `path` to what `typed` says, as the node's type allows.
    pub fn set_value(&mut self, path: &TreePath, typed: &str) -> Result<(), TreeEditError> {
        let (span, shape) = self.value_of(path)?;
        let text = literal(shape, typed)?;
        self.splice(bounds(&span), text.into_bytes())
    }

    /// Rename the key of the node at `path`.
    pub fn rename_key(&mut self, path: &TreePath, name: &str) -> Result<(), TreeEditError> {
        let around = self.tree.around(path)?;
        let key = around.this.key.ok_or(TreeEditError::NotEditable)?;
        let text = serde_json::to_string(name).map_err(|_| TreeEditError::NotEditable)?;
        self.splice(bounds(&key), text.into_bytes())
    }

    /// Take the node at `path` out of its container, with the comma and the white space that led
    /// to it.
    pub fn delete(&mut self, path: &TreePath) -> Result<(), TreeEditError> {
        let around = self.tree.around(path).map_err(|e| {
            if e == TextError::NoSuchNode {
                TreeEditError::NotEditable
            } else {
                TreeEditError::Text(e)
            }
        })?;
        let this_end = around.this.value.end as usize; // a document in memory
        let range = match (&around.prev, &around.next) {
            (_, Some(next)) => start_of(&around.this)..start_of(next),
            (Some(prev), None) => prev.value.end as usize..this_end, // a document in memory
            (None, None) => match around.parent.level.as_ref().map(|level| level.form) {
                Some(Form::Stream) => start_of(&around.this)..this_end,
                Some(Form::Object | Form::Array) | None => {
                    // The container is left with nothing between its brackets.
                    let span = &around.parent.span;
                    span.start as usize + 1..(span.end as usize).saturating_sub(1)
                }
            },
        };
        self.splice(range, Vec::new())
    }

    /// Add a node below the one at `path`: a sibling after it, written with the separator the
    /// document uses between its nodes. A container with nothing in it takes the node inside
    /// itself, and the root takes it after its last. Returns the path of the new node.
    pub fn insert_below(&mut self, path: &TreePath) -> Result<TreePath, TreeEditError> {
        let here = self.tree.row(path)?;
        let count = here.children.0;
        let container = matches!(here.kind, super::NodeKind::Object | super::NodeKind::Array);
        match (path.split_last(), container, count) {
            (_, true, 0) => self.insert_first(path),
            (None, true, _) => {
                let last = count.saturating_sub(1);
                self.insert_after(&path.child(last), last, path)
            }
            (None, false, _) => Err(TreeEditError::NotEditable),
            (Some((index, parent)), _, _) => self.insert_after(path, index, &parent),
        }
    }

    /// The first node of the empty container at `path`.
    fn insert_first(&mut self, path: &TreePath) -> Result<TreePath, TreeEditError> {
        let (span, shape) = self.value_of(path)?;
        let text: &[u8] = match shape {
            Shape::Object => b"\"key\": null",
            Shape::Array => b"null",
            Shape::String
            | Shape::Number
            | Shape::True
            | Shape::False
            | Shape::Null
            | Shape::Unreadable => return Err(TreeEditError::NotEditable),
        };
        let at = span.start as usize + 1; // a document in memory
        self.splice(at..at, text.to_vec())?;
        Ok(path.child(0))
    }

    /// A node after the one at position `index` of `parent`.
    fn insert_after(
        &mut self,
        path: &TreePath,
        index: u32,
        parent: &TreePath,
    ) -> Result<TreePath, TreeEditError> {
        let around = self.tree.around(path)?;
        let this_start = start_of(&around.this);
        let this_end = around.this.value.end as usize; // a document in memory
        let between =
            |from: usize, to: usize| self.bytes.get(from..to).unwrap_or_default().to_vec();
        let form = around.parent.level.as_ref().map(|level| level.form);
        let separator = match (&around.prev, &around.next) {
            (Some(prev), _) => between(prev.value.end as usize, this_start),
            (None, Some(next)) => between(this_end, start_of(next)),
            (None, None) => match form {
                Some(Form::Stream) | None => b"\n".to_vec(),
                Some(Form::Object | Form::Array) => {
                    let mut lead = b",".to_vec();
                    lead.extend(between(around.parent.span.start as usize + 1, this_start));
                    lead
                }
            },
        };
        let mut added = separator;
        match &around.this.key {
            Some(key) => {
                let name =
                    serde_json::to_string(NEW_KEY).map_err(|_| TreeEditError::NotEditable)?;
                added.extend_from_slice(name.as_bytes());
                added.extend(between(key.end as usize, around.this.value.start as usize));
                added.extend_from_slice(b"null");
            }
            None => added.extend_from_slice(b"null"),
        }
        self.splice(this_end..this_end, added)?;
        Ok(parent.child(index.saturating_add(1)))
    }

    /// Take back the last edit.
    pub fn undo(&mut self) -> Undid {
        let Some(step) = self.log.undo().cloned() else {
            return Undid::Nothing;
        };
        self.reverse(&step)
    }

    /// Do the edit taken back again.
    pub fn redo(&mut self) -> Undid {
        let Some(step) = self.log.redo().cloned() else {
            return Undid::Nothing;
        };
        match self.apply(&step) {
            Ok(()) => Undid::Something,
            Err(_) => Undid::Nothing,
        }
    }

    fn reverse(&mut self, step: &Splice) -> Undid {
        let back = Splice {
            at: step.at,
            removed: step.added.clone(),
            added: step.removed.clone(),
        };
        match self.apply(&back) {
            Ok(()) => Undid::Something,
            Err(_) => Undid::Nothing,
        }
    }

    /// How many edits can be taken back.
    #[must_use]
    pub fn undoable(&self) -> usize {
        self.log.undoable()
    }

    /// How many edits can be done again.
    #[must_use]
    pub fn redoable(&self) -> usize {
        self.log.redoable()
    }

    /// The state of the edits now.
    #[must_use]
    pub fn stamp(&self) -> EditStamp {
        self.log.stamp()
    }

    /// Whether the document has changes that are not saved.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.log.is_modified()
    }

    /// The document as of `stamp` is on disk.
    pub fn mark_saved(&mut self, stamp: EditStamp) {
        self.log.mark_saved(stamp);
    }
}
