//! Writing an edited table back as the file it came from. The file's own bytes are copied over
//! whatever was not touched, so a row nobody edited is the same bytes it was, its quoting, its
//! line ending and the file's byte-order mark included, and a diff shows only what changed. A cell
//! that was edited is written in place of the cell, quoted when the cell had been or when the text
//! needs it; a row that was added is written with the file's own line ending; a run of rows taken
//! away is a run of bytes not copied.

use super::edit::{Op, TableEdits};
use super::indexed::{Event, Indexed, State, WINDOW_ROWS, step};
use super::{Rows, Table};
use crate::bytes::read_range;
use crate::encoding::{Coverage, TextCodec};
use crate::error::TextError;
use anyview_core::{ReadAt, TextEncoding};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// How much of the file is read at a time while it is copied.
const COPY_CHUNK: u64 = 4 * 1024 * 1024;

/// How many bytes at the start of the file the line ending is looked for in.
const EOL_LOOK: u64 = 64 * 1024;

/// Where a record is in the file: its first byte, and where its cells end (at its line break, or
/// at the end of the bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place {
    start: u64,
    end: u64,
}

fn refusal(reason: &str) -> TextError {
    TextError::Table {
        reason: reason.to_owned(),
    }
}

impl Table {
    /// Whether the table can be written back as its file: it is a delimited file read whole, in
    /// UTF-8. A workbook, a table parsed from text and a file in another encoding cannot.
    #[must_use]
    pub fn can_write(&self) -> bool {
        match &self.rows {
            Rows::Indexed(indexed) => {
                self.coverage == Coverage::Whole
                    && indexed.codec() == TextCodec::Unicode(TextEncoding::Utf8)
            }
            Rows::Held(_) => false,
        }
    }

    /// The file's bytes with `edits` made to them. Blocking: the file is read through once.
    pub fn write_edited(&self, edits: &TableEdits) -> Result<Vec<u8>, TextError> {
        let Rows::Indexed(indexed) = &self.rows else {
            return Err(refusal("the table is not a file"));
        };
        if !self.can_write() {
            return Err(refusal("the file cannot be written back as it is"));
        }
        let source = indexed.source();
        // A file that is not the length it was indexed at, or has another modification time, was
        // changed since: the offsets are not where they were.
        if source.len().0 != indexed.index().end() || !indexed.is_current() {
            return Err(refusal("the file changed since it was opened"));
        }
        let (sep, skip) = (indexed.separator(), indexed.skip());
        let ops = edits.ops();
        let mut wanted: BTreeSet<u32> = BTreeSet::new();
        wanted.insert(skip);
        wanted.insert(skip + indexed.total());
        for op in &ops {
            match op {
                Op::Keep(rows) => {
                    wanted.insert(skip + rows.start);
                    wanted.insert(skip + rows.end);
                }
                Op::Edit(row, _) => {
                    wanted.insert(skip + row);
                    wanted.insert(skip + row + 1);
                }
                Op::Insert(_) => {}
            }
        }
        let places = locate(indexed, &wanted)?;
        let start_of = |record: u32| places.get(&record).map_or(0, |place| place.start);
        let lonely = edits.columns() == 1;
        let eol = line_ending(&**source)?;
        let mut out = Vec::new();
        copy(&**source, 0..start_of(skip), &mut out)?;
        for op in ops {
            match op {
                Op::Keep(rows) => copy(
                    &**source,
                    start_of(skip + rows.start)..start_of(skip + rows.end),
                    &mut out,
                )?,
                Op::Edit(row, cells) => {
                    let record = skip + row;
                    let Some(place) = places.get(&record) else {
                        continue;
                    };
                    let raw = read_range(&**source, place.start..place.end)?;
                    out.extend(rewrite(&raw, sep, &cells, lonely));
                    copy(&**source, place.end..start_of(record + 1), &mut out)?;
                }
                Op::Insert(cells) => {
                    if !out.is_empty() && !matches!(out.last(), Some(b'\n' | b'\r')) {
                        out.extend_from_slice(&eol);
                    }
                    out.extend(written(&cells, sep, lonely));
                    out.extend_from_slice(&eol);
                }
            }
        }
        Ok(out)
    }
}

/// Copy `range` of `source` onto `out`, a chunk at a time.
fn copy(source: &dyn ReadAt, range: Range<u64>, out: &mut Vec<u8>) -> Result<(), TextError> {
    let mut at = range.start;
    while at < range.end {
        let to = (at + COPY_CHUNK).min(range.end);
        let chunk = read_range(source, at..to)?;
        if chunk.is_empty() {
            break;
        }
        at += chunk.len() as u64; // a usize fits a u64
        out.extend(chunk);
    }
    Ok(())
}

/// The line ending the file uses: the first one in its start, else a line feed.
fn line_ending(source: &dyn ReadAt) -> Result<Vec<u8>, TextError> {
    let head = read_range(source, 0..EOL_LOOK.min(source.len().0))?;
    let ending = match head.iter().position(|byte| matches!(byte, b'\r' | b'\n')) {
        Some(at) if head.get(at) == Some(&b'\r') && head.get(at + 1) == Some(&b'\n') => {
            b"\r\n".to_vec()
        }
        Some(at) if head.get(at) == Some(&b'\r') => b"\r".to_vec(),
        Some(_) | None => b"\n".to_vec(),
    };
    Ok(ending)
}

/// Where the records numbered in `wanted` are. A number past the last record is the end of the
/// bytes. Each record is found by reading the stretch of the file between two recorded offsets.
fn locate(indexed: &Indexed, wanted: &BTreeSet<u32>) -> Result<BTreeMap<u32, Place>, TextError> {
    let index = indexed.index();
    let mut places = BTreeMap::new();
    let mut windows: BTreeSet<u32> = BTreeSet::new();
    for &number in wanted {
        if number >= index.records {
            places.insert(
                number,
                Place {
                    start: index.end(),
                    end: index.end(),
                },
            );
        } else {
            windows.insert(number / WINDOW_ROWS);
        }
    }
    for window in windows {
        let at = window as usize; // a u32 fits a usize
        let Some(&from) = index.starts().get(at) else {
            continue;
        };
        let to = index.starts().get(at + 1).copied().unwrap_or(index.end());
        let bytes = read_range(&**indexed.source(), from..to)?;
        let mut state = State::Between;
        let mut next = window * WINDOW_ROWS;
        let mut current: Option<(u32, u64)> = None;
        for (i, byte) in bytes.iter().enumerate() {
            let (after, event) = step(state, Some(*byte), indexed.separator());
            state = after;
            let here = from + i as u64; // a usize fits a u64
            match event {
                Event::Start | Event::StartAfterSeparator => {
                    current = Some((next, here));
                    next += 1;
                }
                Event::End => {
                    if let Some((number, start)) = current.take()
                        && wanted.contains(&number)
                    {
                        places.insert(number, Place { start, end: here });
                    }
                }
                Event::Separator | Event::Nothing => {}
            }
        }
        if let Some((number, start)) = current.take()
            && wanted.contains(&number)
        {
            places.insert(
                number,
                Place {
                    start,
                    end: from + bytes.len() as u64, // a usize fits a u64
                },
            );
        }
    }
    Ok(places)
}

/// The cells of one record's bytes, as the ranges of the bytes each is written in.
fn fields(raw: &[u8], separator: u8) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut from = 0;
    let mut state = State::Between;
    for (at, byte) in raw.iter().enumerate() {
        let (after, event) = step(state, Some(*byte), separator);
        state = after;
        match event {
            Event::StartAfterSeparator | Event::Separator => {
                spans.push(from..at);
                from = at + 1;
            }
            Event::Start | Event::End | Event::Nothing => {}
        }
    }
    spans.push(from..raw.len());
    spans
}

/// `text` as a cell: quoted when the cell it replaces was, when it needs to be, or when it is the
/// whole of a one-column record and empty (which would be a blank line).
fn cell_bytes(text: &str, was_quoted: bool, lonely: bool, separator: u8) -> Vec<u8> {
    let needs = text
        .bytes()
        .any(|byte| byte == separator || matches!(byte, b'"' | b'\n' | b'\r'))
        || (lonely && text.is_empty());
    if !(was_quoted || needs) {
        return text.as_bytes().to_vec();
    }
    let mut out = vec![b'"'];
    for byte in text.bytes() {
        if byte == b'"' {
            out.push(b'"');
        }
        out.push(byte);
    }
    out.push(b'"');
    out
}

/// A record's bytes with the cells in `changed` written in place of the ones it had, the other
/// cells' bytes as they were. A record shorter than a changed column grows to it.
fn rewrite(raw: &[u8], separator: u8, changed: &BTreeMap<u32, String>, lonely: bool) -> Vec<u8> {
    let spans = fields(raw, separator);
    let last = changed
        .keys()
        .next_back()
        .map_or(0, |column| *column as usize) // a u32 fits a usize
        .max(spans.len().saturating_sub(1));
    let mut out = Vec::with_capacity(raw.len());
    for column in 0..=last {
        if column > 0 {
            out.push(separator);
        }
        let original = spans.get(column).map(|span| &raw[span.clone()]);
        match (changed.get(&(column as u32)), original) {
            // a column is far below u32's range
            (Some(text), original) => {
                let quoted = original.is_some_and(|bytes| bytes.first() == Some(&b'"'));
                out.extend(cell_bytes(text, quoted, lonely, separator));
            }
            (None, Some(bytes)) => out.extend_from_slice(bytes),
            (None, None) => {}
        }
    }
    out
}

/// A record that was added, from its cells.
fn written(cells: &[String], separator: u8, lonely: bool) -> Vec<u8> {
    let mut out = Vec::new();
    for (column, text) in cells.iter().enumerate() {
        if column > 0 {
            out.push(separator);
        }
        out.extend(cell_bytes(text, false, lonely, separator));
    }
    out
}
