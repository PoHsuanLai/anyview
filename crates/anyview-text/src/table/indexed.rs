//! A delimited file indexed by record: where every 64th record starts, found by one streaming pass
//! over the bytes, the way `TextLines` indexes lines. A window of rows is then parsed from the
//! nearest offset when it is asked for and the file is never held. A record is not a line, because
//! a quoted cell may hold line breaks, so the pass follows the quotes.

use crate::bytes::read_range;
use crate::encoding::{Coverage, TextCodec};
use crate::error::TextError;
use anyview_core::work::Interrupt;
use anyview_core::{ReadAt, TextEncoding};
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Records in a window: the stretch between two recorded offsets.
pub(super) const WINDOW_ROWS: u32 = 64;

/// Bytes read at a time while indexing. A multiple of 2 so a UTF-16 unit never splits.
const CHUNK: u64 = 64 * 1024;

/// How many parsed windows are kept for the next ask.
const WINDOWS_KEPT: usize = 4;

/// The records of one window, parsed.
type Window = Arc<Vec<Vec<String>>>;

/// Where a scan is in a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Between records: nothing of the next has been read.
    Between,
    /// At the start of a field.
    FieldStart,
    /// Inside a field that is not quoted.
    InField,
    /// Inside a quoted field, where separators and line breaks are text.
    InQuoted,
    /// Just past a quote inside a quoted field: another quote makes it a quote, anything else
    /// closes the field.
    QuoteSeen,
}

/// What a byte did to the records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Nothing,
    /// A record starts here, in its first field.
    Start,
    /// A record starts here with an empty first field, and a second begins.
    StartAfterSeparator,
    /// A field ends and the next begins.
    Separator,
    /// The record ends.
    End,
}

/// The ASCII character a unit of `bytes` at `at` is, when it is one. A unit of another character
/// (or a byte of a multi-byte one) is `None` or matches nothing.
fn unit_char(codec: TextCodec, bytes: &[u8], at: usize) -> Option<u8> {
    match codec {
        TextCodec::Unicode(TextEncoding::Utf16Le) => match (bytes.get(at), bytes.get(at + 1)) {
            (Some(&low), Some(&0)) => Some(low),
            _ => None,
        },
        TextCodec::Unicode(TextEncoding::Utf16Be) => match (bytes.get(at), bytes.get(at + 1)) {
            (Some(&0), Some(&low)) => Some(low),
            _ => None,
        },
        TextCodec::Unicode(TextEncoding::Utf8) | TextCodec::Windows1252 | TextCodec::Legacy(_) => {
            bytes.get(at).copied()
        }
    }
}

/// The scan's next state, and what `ch` did. A line break outside quotes ends a record (`\r`,
/// `\n` or `\r\n`), and a line with nothing on it is no record, as the `csv` reader has it; a
/// quote opens a quoted field only at the start of a field.
fn step(state: State, ch: Option<u8>, separator: u8) -> (State, Event) {
    let line_break = matches!(ch, Some(b'\r' | b'\n'));
    let quote = ch == Some(b'"');
    let split = ch == Some(separator);
    match state {
        State::Between if line_break => (State::Between, Event::Nothing),
        State::Between if quote => (State::InQuoted, Event::Start),
        State::Between if split => (State::FieldStart, Event::StartAfterSeparator),
        State::Between => (State::InField, Event::Start),
        State::InQuoted if quote => (State::QuoteSeen, Event::Nothing),
        State::InQuoted => (State::InQuoted, Event::Nothing),
        State::QuoteSeen if quote => (State::InQuoted, Event::Nothing),
        State::FieldStart | State::InField | State::QuoteSeen if line_break => {
            (State::Between, Event::End)
        }
        State::FieldStart | State::InField | State::QuoteSeen if split => {
            (State::FieldStart, Event::Separator)
        }
        State::FieldStart if quote => (State::InQuoted, Event::Nothing),
        State::FieldStart | State::InField | State::QuoteSeen => (State::InField, Event::Nothing),
    }
}

/// Where records start in one file's bytes, past its byte-order mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RecordIndex {
    /// The offset of record 0, 64, 128, ….
    starts: Vec<u64>,
    /// How many records the bytes hold.
    pub records: u32,
    /// The most cells any record has.
    pub widest: u32,
    /// Where the bytes counted end.
    end: u64,
}

impl RecordIndex {
    /// Streams `source` once between the offsets of `span` (the first byte after any byte-order
    /// mark, and the end). Ends with [`TextError::Interrupted`] when `interrupt` says so, once per chunk. When `coverage` says the file goes on past the end, a record the end
    /// cuts short is left out.
    pub(super) fn build(
        source: &dyn ReadAt,
        codec: TextCodec,
        separator: u8,
        span: (u64, u64),
        coverage: Coverage,
        interrupt: &dyn Interrupt,
    ) -> Result<Self, TextError> {
        let (content, end) = span;
        let unit = codec.unit();
        let mut starts = Vec::new();
        let mut records = 0_u32;
        let (mut fields, mut widest) = (0_u32, 0_u32);
        let mut state = State::Between;
        let mut at = content;
        while at < end {
            if interrupt.interrupted() {
                return Err(TextError::Interrupted);
            }
            let chunk = read_range(source, at..(at + CHUNK).min(end))?;
            if chunk.is_empty() {
                break;
            }
            for i in (0..chunk.len()).step_by(unit) {
                let (next, event) = step(state, unit_char(codec, &chunk, i), separator);
                state = next;
                match event {
                    Event::Nothing => {}
                    Event::Start | Event::StartAfterSeparator => {
                        if records.is_multiple_of(WINDOW_ROWS) {
                            starts.push(at + i as u64); // a usize fits a u64
                        }
                        records = records.saturating_add(1);
                        fields = if event == Event::Start { 1 } else { 2 };
                    }
                    Event::Separator => fields = fields.saturating_add(1),
                    Event::End => {
                        widest = widest.max(fields);
                        fields = 0;
                    }
                }
            }
            at += chunk.len() as u64; // a usize fits a u64
        }
        if state != State::Between {
            match coverage {
                Coverage::Whole => widest = widest.max(fields),
                Coverage::Prefix => {
                    // The end cut the last record short: it is not counted.
                    records = records.saturating_sub(1);
                    if records.is_multiple_of(WINDOW_ROWS) {
                        starts.pop();
                    }
                }
            }
        }
        Ok(RecordIndex {
            starts,
            records,
            widest,
            end,
        })
    }
}

/// The type of the cache of parsed windows.
type Windows = Mutex<VecDeque<(u32, Window)>>;

/// A file's records, read in windows. Cloning shares the bytes, the index and the parsed windows.
#[derive(Debug, Clone)]
pub(super) struct Indexed {
    source: Arc<dyn ReadAt>,
    codec: TextCodec,
    separator: u8,
    index: Arc<RecordIndex>,
    /// The records before the first row: 1 when the first record is the header.
    skip: u32,
    windows: Arc<Windows>,
}

impl Indexed {
    pub(super) fn new(
        source: Arc<dyn ReadAt>,
        codec: TextCodec,
        separator: u8,
        index: RecordIndex,
    ) -> Self {
        Indexed {
            source,
            codec,
            separator,
            index: Arc::new(index),
            skip: 0,
            windows: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// The same records with the first `skip` of them not counted as rows.
    pub(super) fn with_skip(self, skip: u32) -> Self {
        Indexed { skip, ..self }
    }

    /// How many records the file holds.
    pub(super) fn records(&self) -> u32 {
        self.index.records
    }

    /// How many rows there are: the records past the header.
    pub(super) fn total(&self) -> u32 {
        self.index.records.saturating_sub(self.skip)
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<(u32, Window)>> {
        self.windows.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records `64 * k` onward, `64` of them: parsed, or the one parsed last time.
    pub(super) fn window(&self, k: u32) -> Result<Window, TextError> {
        let hit = self
            .lock()
            .iter()
            .find(|(at, _)| *at == k)
            .map(|(_, rows)| Arc::clone(rows));
        if let Some(hit) = hit {
            return Ok(hit);
        }
        let parsed = Arc::new(self.parse(k)?);
        let mut windows = self.lock();
        if windows.len() >= WINDOWS_KEPT {
            windows.pop_front();
        }
        windows.push_back((k, Arc::clone(&parsed)));
        Ok(parsed)
    }

    fn parse(&self, k: u32) -> Result<Vec<Vec<String>>, TextError> {
        let Some(&from) = self.index.starts.get(k as usize) else {
            return Ok(Vec::new()); // a u32 fits a usize
        };
        let to = self
            .index
            .starts
            .get(k as usize + 1) // a u32 fits a usize
            .copied()
            .unwrap_or(self.index.end);
        let bytes = read_range(&*self.source, from..to)?;
        let text = self.codec.decode(&bytes);
        let wanted = (self.index.records - k * WINDOW_ROWS).min(WINDOW_ROWS) as usize; // a u32 fits a usize
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.separator)
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut rows = Vec::with_capacity(wanted);
        for record in reader.records().take(wanted) {
            let record = record.map_err(|e| TextError::Table {
                reason: e.to_string(),
            })?;
            rows.push(record.iter().map(str::to_owned).collect());
        }
        Ok(rows)
    }

    /// The rows in `range`, cut to the rows that exist.
    pub(super) fn rows(&self, range: Range<u32>) -> Result<Vec<Vec<String>>, TextError> {
        let end = range.end.min(self.total());
        let start = range.start.min(end);
        let mut out = Vec::with_capacity((end - start).min(WINDOW_ROWS) as usize); // a u32 fits a usize
        let last = end + self.skip;
        let mut record = start + self.skip;
        while record < last {
            let window = self.window(record / WINDOW_ROWS)?;
            let within = record % WINDOW_ROWS;
            let take = (last - record).min(WINDOW_ROWS - within);
            let from = within as usize; // a u32 fits a usize
            out.extend_from_slice(window.get(from..from + take as usize).unwrap_or_default());
            record += take;
        }
        Ok(out)
    }

    /// The cell at `row` and `column`: empty where the row is shorter, or cannot be read.
    pub(super) fn cell(&self, row: u32, column: usize) -> String {
        if row >= self.total() {
            return String::new();
        }
        let record = row + self.skip;
        let Ok(window) = self.window(record / WINDOW_ROWS) else {
            return String::new();
        };
        window
            .get((record % WINDOW_ROWS) as usize) // a u32 fits a usize
            .and_then(|cells| cells.get(column))
            .cloned()
            .unwrap_or_default()
    }
}
