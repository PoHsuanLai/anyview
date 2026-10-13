//! The children of one container, indexed sparsely: the offset of every 64th child (sooner when
//! the children are large), found by one scan. A window of children is then read from the nearest
//! offset, so a million-item array costs a count and a few thousand offsets, and a window of it
//! the children in the window.

use super::cursor::Cursor;
use super::grammar::{
    Child, Fault, Form, MAX_DEPTH, Shape, Why, bad, closing, eof, read_child, unexpected,
};
use crate::encoding::Coverage;
use std::ops::Range;

/// Children between two recorded offsets, at most.
const STRIDE: u32 = 64;

/// Bytes between two recorded offsets, at least, after which one is recorded whatever the count.
const SPAN: u64 = 256 * 1024;

/// How a container's children are found: how many there are, and where some of them start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Level {
    pub form: Form,
    /// How many children were read.
    pub count: u32,
    /// The index and offset of some children, ascending; the first child is always one.
    anchors: Vec<(u32, u64)>,
}

impl Level {
    fn new(form: Form) -> Self {
        Level {
            form,
            count: 0,
            anchors: Vec::new(),
        }
    }

    /// Takes in the child that starts at `start`, the next one.
    fn commit(&mut self, start: u64) {
        let due = match self.anchors.last() {
            None => true,
            Some(&(index, offset)) => {
                self.count - index >= STRIDE || start.saturating_sub(offset) >= SPAN
            }
        };
        if due {
            self.anchors.push((self.count, start));
        }
        self.count = self.count.saturating_add(1);
    }

    /// The recorded offset nearest at or before child `index`.
    fn anchor_for(&self, index: u32) -> Option<(u32, u64)> {
        let after = self.anchors.partition_point(|(at, _)| *at <= index);
        after
            .checked_sub(1)
            .and_then(|i| self.anchors.get(i).copied())
    }
}

/// Reads the next value of a JSON Lines file. A line that is not a value is a child of its own, an
/// unreadable one holding the rest of the line, so one bad line does not fail the file and the
/// lines after it are still read. When `reach` says the bytes are a start, a line cut by their end
/// is no child yet and the fault says the bytes ended.
fn read_line(c: &mut Cursor<'_>, depth: usize, reach: Coverage) -> Result<Child, Fault> {
    let start = c.pos();
    match read_child(c, Form::Stream, depth) {
        Ok(child) => Ok(child),
        Err(fault) if fault.why == Why::Eof && reach == Coverage::Prefix => Err(fault),
        Err(_) => {
            c.seek(start);
            loop {
                let rest = c.rest();
                if rest.is_empty() {
                    break;
                }
                if let Some(at) = rest.iter().position(|byte| *byte == b'\n') {
                    c.advance(at);
                    break;
                }
                let all = rest.len();
                c.advance(all);
            }
            if c.peek().is_none() && reach == Coverage::Prefix {
                return Err(eof(c));
            }
            Ok(Child {
                key: None,
                value: start..c.pos(),
                shape: Shape::Unreadable,
            })
        }
    }
}

/// Reads the children of the container at the cursor (at its opening bracket; a stream has none,
/// and starts at its first value), checking every one is well formed. `depth` is how many
/// containers enclose it. When `reach` says the bytes are only the start of a file, a child cut
/// off by their end is left out and the scan says it saw a prefix.
pub(super) fn scan_level(
    c: &mut Cursor<'_>,
    form: Form,
    depth: usize,
    reach: Coverage,
) -> Result<(Level, Coverage), Fault> {
    let mut level = Level::new(form);
    match fill(c, &mut level, depth, reach) {
        Ok(()) => Ok((level, Coverage::Whole)),
        Err(Fault { why: Why::Eof, .. }) if reach == Coverage::Prefix => {
            Ok((level, Coverage::Prefix))
        }
        Err(fault) => Err(fault),
    }
}

fn fill(c: &mut Cursor<'_>, level: &mut Level, depth: usize, reach: Coverage) -> Result<(), Fault> {
    let form = level.form;
    let close = closing(form);
    if let Some(close) = close {
        if depth >= MAX_DEPTH {
            return Err(bad(c.pos(), "recursion limit exceeded"));
        }
        c.bump();
        c.skip_ws();
        if c.peek() == Some(close) {
            c.bump();
            return Ok(());
        }
    } else {
        c.skip_ws();
        if c.peek().is_none() {
            return Ok(());
        }
    }
    loop {
        let start = c.pos();
        if form == Form::Stream {
            read_line(c, depth + 1, reach)?;
        } else {
            read_child(c, form, depth + 1)?;
        }
        let more = match close {
            Some(close) => {
                c.skip_ws();
                match c.peek() {
                    Some(b',') => {
                        c.bump();
                        c.skip_ws();
                        true
                    }
                    Some(byte) if byte == close => {
                        c.bump();
                        false
                    }
                    _ => return Err(unexpected(c, "expected `,` or a closing bracket")),
                }
            }
            None => {
                let skipped = c.skip_ws();
                if c.peek().is_some() {
                    true
                } else if reach == Coverage::Prefix {
                    // The bytes end here, and the file goes on. A value is whole only when white
                    // space follows it: `12` could be `123`.
                    if skipped > 0 {
                        level.commit(start);
                    }
                    return Err(eof(c));
                } else {
                    false
                }
            }
        };
        level.commit(start);
        if !more {
            return Ok(());
        }
    }
}

/// The children of `level` in `window`, cut to the children that exist, each with its index.
/// `depth` is how many containers enclose the level's container.
pub(super) fn read_window(
    c: &mut Cursor<'_>,
    level: &Level,
    window: Range<u32>,
    depth: usize,
) -> Result<Vec<(u32, Child)>, Fault> {
    let end = window.end.min(level.count);
    let first = window.start.min(end);
    if first >= end {
        return Ok(Vec::new());
    }
    let Some((mut index, offset)) = level.anchor_for(first) else {
        return Ok(Vec::new());
    };
    c.seek(offset);
    let mut found = Vec::new();
    loop {
        let child = if level.form == Form::Stream {
            read_line(c, depth + 1, Coverage::Whole)?
        } else {
            read_child(c, level.form, depth + 1)?
        };
        if index >= first {
            found.push((index, child));
        }
        index += 1;
        if index >= end {
            return Ok(found);
        }
        c.skip_ws();
        if level.form != Form::Stream {
            if c.peek() != Some(b',') {
                return Err(unexpected(c, "expected `,`"));
            }
            c.bump();
            c.skip_ws();
        }
    }
}
