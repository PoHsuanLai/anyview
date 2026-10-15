//! JSON and JSON Lines as a tree, opened one level at a time.
//!
//! A document is scanned once, streaming, to check it is well formed and to index the children of
//! its top level (the offset of every 64th, like `TextLines` does for lines). Nothing is parsed
//! into a tree: a node's children are read from the file when they are asked for, so a document
//! of any size costs its index and the rows on screen, a container is indexed when it is first
//! shown, and a number reads as it was written.

mod cursor;
mod edit;
mod grammar;
mod level;
mod rows;
mod visible;

#[cfg(test)]
mod tests;

pub use edit::{TreeEditError, TreeSession};
pub use rows::{ChildCount, NodeKind, RowLabel, TreeRow};
pub use visible::{Openness, VisibleRow};

use crate::bytes::read_range;
use crate::encoding::{Coverage, DETECT_BYTES, TextCodec, detect};
use crate::error::TextError;
use anyview_core::work::Interrupt;
use anyview_core::{
    ByteLen, Deadline, Input, LineIndex, ReadAt, TextEncoding, TreeFormat, TreePath,
};
use cursor::Cursor;
use grammar::{Child, Fault, Form, Shape, Why, bad, shape_at, skip_value};
use level::{Level, read_window, scan_level};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// How many indexed containers a tree keeps; past this it starts again.
const LEVELS_KEPT: usize = 512;

/// The most bytes of a string that are read to show it; a row shows far fewer characters.
const STRING_READ: u64 = 2048;

/// The most bytes of a number that are read to show it.
const NUMBER_READ: u64 = 256;

/// A JSON document, or the values of a JSON Lines file as the items of an array, read from its
/// bytes as they are asked for. Cloning shares the bytes and the index.
#[derive(Debug, Clone)]
pub struct Tree {
    format: TreeFormat,
    store: Arc<Store>,
}

#[derive(Debug)]
struct Store {
    source: Arc<dyn ReadAt>,
    /// How strings are decoded.
    codec: TextCodec,
    /// What the file is in, as a pane lists it: differs from `codec` for UTF-16, which is held
    /// as UTF-8.
    encoding: TextCodec,
    /// Where the document starts: past any byte-order mark.
    origin: u64,
    /// Where the bytes that count end.
    end: u64,
    coverage: Coverage,
    root: Root,
    /// The containers indexed so far, by where they are.
    levels: Mutex<HashMap<TreePath, Arc<Level>>>,
}

/// The document's top-level value.
#[derive(Debug)]
struct Root {
    shape: Shape,
    span: Range<u64>,
    level: Option<Arc<Level>>,
}

/// A node found by its path.
struct Located {
    label: RowLabel,
    shape: Shape,
    span: Range<u64>,
    level: Option<Arc<Level>>,
}

/// The line and column of a fault, as an error.
fn fault_error(source: &dyn ReadAt, fault: Fault, origin: u64) -> TextError {
    let (line, column) = position(source, origin, fault.at);
    let reason = match fault.why {
        Why::Eof => "unexpected end of the document".to_owned(),
        Why::Bad(reason) => reason.to_owned(),
    };
    TextError::Json {
        line: LineIndex(line),
        column,
        reason,
    }
}

/// The zero-based line and one-based column, in bytes, of offset `at` in a document that starts at
/// `origin`.
fn position(source: &dyn ReadAt, origin: u64, at: u64) -> (u32, u32) {
    const CHUNK: u64 = 64 * 1024;
    let mut line = 0_u32;
    let mut line_start = origin;
    let mut from = origin;
    while from < at {
        let Ok(chunk) = read_range(source, from..(from + CHUNK).min(at)) else {
            break;
        };
        if chunk.is_empty() {
            break;
        }
        for (i, byte) in chunk.iter().enumerate() {
            if *byte == b'\n' {
                line = line.saturating_add(1);
                line_start = from + i as u64 + 1; // a usize fits a u64
            }
        }
        from += chunk.len() as u64; // a usize fits a u64
    }
    let column = at.saturating_sub(line_start) + 1;
    (line, u32::try_from(column).unwrap_or(u32::MAX))
}

/// Scans a JSON document: one value, then nothing but white space.
fn document(c: &mut Cursor<'_>, cut: Coverage) -> Result<(Root, Coverage), Fault> {
    c.skip_ws();
    let start = c.pos();
    let shape = shape_at(c)?;
    let (level, reached) = match shape {
        Shape::Object | Shape::Array => {
            let form = if shape == Shape::Object {
                Form::Object
            } else {
                Form::Array
            };
            let (level, reached) = scan_level(c, form, 0, cut)?;
            (Some(Arc::new(level)), reached)
        }
        Shape::String
        | Shape::Number
        | Shape::True
        | Shape::False
        | Shape::Null
        | Shape::Unreadable => {
            skip_value(c, 0)?;
            (None, Coverage::Whole)
        }
    };
    let span = start..c.pos();
    if reached == Coverage::Whole {
        c.skip_ws();
        if c.peek().is_some() {
            return Err(bad(c.pos(), "trailing characters"));
        }
    }
    let root = Root { shape, span, level };
    Ok((root, reached))
}

/// Scans a JSON Lines file: values one after another, a blank line between them or not.
fn lines(c: &mut Cursor<'_>, cut: Coverage) -> Result<(Root, Coverage), Fault> {
    let (level, reached) = scan_level(c, Form::Stream, 0, cut)?;
    let root = Root {
        shape: Shape::Array,
        span: 0..0,
        level: Some(Arc::new(level)),
    };
    Ok((root, reached))
}

/// Scans `source` from `origin` to `end`. `cut` says whether the file goes on past `end`.
fn scan_root(
    source: &dyn ReadAt,
    origin: u64,
    end: u64,
    format: TreeFormat,
    cut: Coverage,
    interrupt: &dyn Interrupt,
) -> Result<(Root, Coverage), TextError> {
    let mut c = Cursor::new(source, origin, end).until(interrupt);
    let scanned = match format {
        TreeFormat::Json => document(&mut c, cut),
        TreeFormat::JsonLines => lines(&mut c, cut),
    };
    if let Some(error) = c.failure() {
        return Err(error);
    }
    scanned.map_err(|fault| match fault.why {
        // A single value that does not end within the bytes is more than a peek can show.
        Why::Eof if cut == Coverage::Prefix => TextError::JsonOverBudget,
        Why::Eof | Why::Bad(_) => fault_error(source, fault, origin),
    })
}

impl Tree {
    /// The document in `text`: one JSON value for `Json`, the values of its lines for
    /// `JsonLines`. A malformed one names its line and column.
    pub fn parse(text: &str, format: TreeFormat) -> Result<Self, TextError> {
        let end = text.len() as u64; // a usize fits a u64
        let bytes = text.as_bytes().to_vec();
        let utf8 = TextCodec::Unicode(TextEncoding::Utf8);
        Tree::open(
            Arc::new(bytes),
            (utf8, utf8),
            (0, end),
            format,
            Coverage::Whole,
            &Deadline::NEVER,
        )
    }

    /// The document in the file `src` names, scanning at most `limit` bytes of it, whatever its
    /// size: a file longer than that shows its start, and says so in the coverage it returns. A
    /// UTF-16 file is decoded and held, up to `limit`.
    pub fn read(
        src: impl Into<Input>,
        format: TreeFormat,
        limit: ByteLen,
    ) -> Result<(Self, Coverage), TextError> {
        Tree::read_until(src, format, limit, &Deadline::NEVER)
    }

    /// [`Tree::read`], ended with [`TextError::Interrupted`] when `interrupt` says so: it is asked
    /// each time the scan reads a chunk of the file.
    pub fn read_until(
        src: impl Into<Input>,
        format: TreeFormat,
        limit: ByteLen,
        interrupt: &dyn Interrupt,
    ) -> Result<(Self, Coverage), TextError> {
        if limit.0 == 0 {
            return Err(TextError::NoBudget);
        }
        let input: Input = src.into();
        let source = Arc::clone(input.bytes());
        let len = source.len().0;
        let head = read_range(&*source, 0..DETECT_BYTES as u64)?; // a usize fits a u64
        let looked = if len > head.len() as u64 {
            Coverage::Prefix
        } else {
            Coverage::Whole
        };
        let detected = detect(&head, looked);
        let cut = if len > limit.0 {
            Coverage::Prefix
        } else {
            Coverage::Whole
        };
        let tree = if detected.codec.unit() == 2 {
            let bytes = read_range(&*source, 0..limit.0)?;
            let body = bytes.get(usize::from(detected.mark)..).unwrap_or_default();
            let text = detected.codec.decode(body).into_owned().into_bytes();
            let end = text.len() as u64; // a usize fits a u64
            let codecs = (TextCodec::Unicode(TextEncoding::Utf8), detected.codec);
            Tree::open(Arc::new(text), codecs, (0, end), format, cut, interrupt)?
        } else {
            let span = (u64::from(detected.mark), limit.0.min(len));
            Tree::open(
                source,
                (detected.codec, detected.codec),
                span,
                format,
                cut,
                interrupt,
            )?
        };
        let coverage = tree.store.coverage;
        Ok((tree, coverage))
    }

    /// The document in a file's bytes: the encoding is detected, a byte-order mark skipped.
    pub fn parse_bytes(bytes: &[u8], format: TreeFormat) -> Result<Self, TextError> {
        let detected = detect(bytes, Coverage::Whole);
        let body = bytes.get(usize::from(detected.mark)..).unwrap_or_default();
        if detected.codec.unit() == 2 {
            return Tree::parse(&detected.codec.decode(body), format);
        }
        let end = bytes.len() as u64; // a usize fits a u64
        let span = (u64::from(detected.mark), end);
        let codecs = (detected.codec, detected.codec);
        Tree::open(
            Arc::new(bytes.to_vec()),
            codecs,
            span,
            format,
            Coverage::Whole,
            &Deadline::NEVER,
        )
    }

    /// Scans `source` between the offsets of `span` (where the document starts and ends).
    /// `codecs` are how strings are decoded and what the file is in.
    fn open(
        source: Arc<dyn ReadAt>,
        codecs: (TextCodec, TextCodec),
        span: (u64, u64),
        format: TreeFormat,
        cut: Coverage,
        interrupt: &dyn Interrupt,
    ) -> Result<Self, TextError> {
        let (origin, end) = span;
        let (codec, encoding) = codecs;
        let (root, coverage) = scan_root(&*source, origin, end, format, cut, interrupt)?;
        Ok(Tree {
            format,
            store: Arc::new(Store {
                source,
                codec,
                encoding,
                origin,
                end,
                coverage,
                root,
                levels: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Which format it was read as.
    pub fn format(&self) -> TreeFormat {
        self.format
    }

    /// Whether the tree holds all the document or only its start.
    pub fn coverage(&self) -> Coverage {
        self.store.coverage
    }

    /// The encoding of the file the document was read from.
    pub(crate) fn encoding(&self) -> TextCodec {
        self.store.encoding
    }

    fn cursor(&self, at: u64) -> Cursor<'_> {
        Cursor::new(&*self.store.source, at, self.store.end)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<TreePath, Arc<Level>>> {
        self.store
            .levels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The error for a fault met while reading after the scan: a read that failed, else the
    /// bytes having changed since.
    fn fail(&self, c: &mut Cursor<'_>, fault: Fault) -> TextError {
        c.failure()
            .unwrap_or_else(|| fault_error(&*self.store.source, fault, self.store.origin))
    }

    /// The text of the JSON string in `span`, unescaped; one longer than a row can show is cut.
    fn string_text(&self, span: &Range<u64>) -> Result<String, TextError> {
        let len = span.end.saturating_sub(span.start);
        let read = len.min(STRING_READ);
        let mut bytes = read_range(&*self.store.source, span.start..span.start + read)?;
        if read < len {
            // Cut: drop an escape the cut may have split, and close the quote.
            if let Some(slash) = bytes.iter().rposition(|b| *b == b'\\')
                && slash + 6 > bytes.len()
            {
                bytes.truncate(slash);
            }
            bytes.push(b'"');
        }
        let text = self.store.codec.decode(&bytes);
        Ok(serde_json::from_str::<String>(&text)
            .unwrap_or_else(|_| text.trim_matches('"').to_owned()))
    }

    /// What a scalar row shows: the string unescaped, the number as it was written.
    fn scalar_text(&self, shape: Shape, span: &Range<u64>) -> Result<String, TextError> {
        match shape {
            Shape::String => self.string_text(span),
            Shape::Number => {
                let len = span.end.saturating_sub(span.start);
                let read = len.min(NUMBER_READ);
                let bytes = read_range(&*self.store.source, span.start..span.start + read)?;
                let mut text = String::from_utf8_lossy(&bytes).into_owned();
                if read < len {
                    text.push('…');
                }
                Ok(text)
            }
            Shape::Unreadable => self.raw_text(span),
            Shape::Object | Shape::Array | Shape::True | Shape::False | Shape::Null => {
                Ok(String::new())
            }
        }
    }

    /// The bytes of an unreadable line as text, with its line end cut off; one longer than a row
    /// can show is cut.
    fn raw_text(&self, span: &Range<u64>) -> Result<String, TextError> {
        let len = span.end.saturating_sub(span.start);
        let read = len.min(STRING_READ);
        let bytes = read_range(&*self.store.source, span.start..span.start + read)?;
        let mut text = self.store.codec.decode(&bytes).trim_end().to_owned();
        if read < len {
            text.push('…');
        }
        Ok(text)
    }

    /// The index of the container at `path`, whose value starts at `start`: found from the cache,
    /// else by scanning it.
    fn level_of(&self, path: &TreePath, shape: Shape, start: u64) -> Result<Arc<Level>, TextError> {
        if let Some(level) = self.lock().get(path) {
            return Ok(Arc::clone(level));
        }
        let form = if shape == Shape::Object {
            Form::Object
        } else {
            Form::Array
        };
        let mut c = self.cursor(start);
        let scanned = scan_level(&mut c, form, path.depth(), Coverage::Whole);
        let (level, _) = scanned.map_err(|fault| self.fail(&mut c, fault))?;
        let level = Arc::new(level);
        let mut levels = self.lock();
        if levels.len() >= LEVELS_KEPT {
            levels.clear();
        }
        levels.insert(path.clone(), Arc::clone(&level));
        Ok(level)
    }

    /// What a child is called: its key, or its position.
    fn label_of(&self, index: u32, child: &Child) -> Result<RowLabel, TextError> {
        match &child.key {
            Some(key) => Ok(RowLabel::Key(self.string_text(key)?)),
            None => Ok(RowLabel::Index(index)),
        }
    }

    /// The children of the container at `path` (whose index is `level`) in `window`.
    fn read_children(
        &self,
        path: &TreePath,
        level: &Level,
        window: Range<u32>,
    ) -> Result<Vec<(u32, Child)>, TextError> {
        let mut c = self.cursor(0);
        let found = read_window(&mut c, level, window, path.depth());
        found.map_err(|fault| self.fail(&mut c, fault))
    }

    /// The node at `path`, found by walking down from the root one step at a time, so a path as
    /// deep as the document nests costs no stack.
    fn locate(&self, path: &TreePath) -> Result<Located, TextError> {
        let root = &self.store.root;
        let mut at = Located {
            label: RowLabel::Root,
            shape: root.shape,
            span: root.span.clone(),
            level: root.level.clone(),
        };
        let mut walked = TreePath::root();
        for &step in path.steps() {
            let level = at.level.ok_or(TextError::NoSuchNode)?;
            let found = self.read_children(&walked, &level, step..step.saturating_add(1))?;
            let (index, child) = found.into_iter().next().ok_or(TextError::NoSuchNode)?;
            let label = self.label_of(index, &child)?;
            walked = walked.child(step);
            let level = match child.shape {
                Shape::Object | Shape::Array => {
                    Some(self.level_of(&walked, child.shape, child.value.start)?)
                }
                Shape::String
                | Shape::Number
                | Shape::True
                | Shape::False
                | Shape::Null
                | Shape::Unreadable => None,
            };
            at = Located {
                label,
                shape: child.shape,
                span: child.value,
                level,
            };
        }
        Ok(at)
    }

    /// The row for the node at `path`, labelled as the root when the path is empty and otherwise
    /// by the last step's key or index.
    pub fn row(&self, path: &TreePath) -> Result<TreeRow, TextError> {
        let at = self.locate(path)?;
        let text = self.scalar_text(at.shape, &at.span)?;
        let len = at.level.map_or(0, |level| level.count);
        Ok(rows::row(at.label, at.shape, &text, len))
    }

    /// The rows for the children of the node at `path` in `window`, cut to the children that
    /// exist. A scalar has none.
    pub fn children(&self, path: &TreePath, window: Range<u32>) -> Result<Vec<TreeRow>, TextError> {
        let parent = self.locate(path)?;
        let Some(level) = parent.level else {
            return Ok(Vec::new());
        };
        self.read_children(path, &level, window)?
            .into_iter()
            .map(|(index, child)| -> Result<TreeRow, TextError> {
                let label = self.label_of(index, &child)?;
                match child.shape {
                    Shape::Object | Shape::Array => {
                        let at = path.child(index);
                        let level = self.level_of(&at, child.shape, child.value.start)?;
                        Ok(rows::row(label, child.shape, "", level.count))
                    }
                    Shape::String
                    | Shape::Number
                    | Shape::True
                    | Shape::False
                    | Shape::Null
                    | Shape::Unreadable => {
                        let text = self.scalar_text(child.shape, &child.value)?;
                        Ok(rows::row(label, child.shape, &text, 0))
                    }
                }
            })
            .collect()
    }

    /// The keys of the root object in file order, at most `limit` of them; empty when the root is
    /// not an object.
    pub fn top_level_keys(&self, limit: usize) -> Vec<String> {
        let Some(level) = self
            .store
            .root
            .level
            .as_ref()
            .filter(|level| level.form == Form::Object)
        else {
            return Vec::new();
        };
        let window = 0..u32::try_from(limit).unwrap_or(u32::MAX);
        let Ok(found) = self.read_children(&TreePath::root(), level, window) else {
            return Vec::new();
        };
        found
            .iter()
            .filter_map(|(_, child)| child.key.as_ref())
            .filter_map(|key| self.string_text(key).ok())
            .collect()
    }
}
