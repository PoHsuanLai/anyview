//! CSV and TSV as a table: rows that can be windowed, a header the viewer guesses or is told. A
//! delimited file is indexed by record and its rows are parsed a window at a time (`indexed`); a
//! spreadsheet's sheet, and a table parsed from text, hold their rows.

mod edit;
mod header;
mod indexed;
mod separator;
mod workbook;
mod write;

#[cfg(test)]
mod tests;

use crate::bytes::read_range;
use crate::encoding::{Coverage, DETECT_BYTES, Detected, TextCodec, detect};
use crate::error::TextError;
use anyview_core::work::Interrupt;
use anyview_core::{ByteLen, Deadline, Delimiter, Input};
use header::looks_like_header;
use indexed::{Indexed, RecordIndex, WINDOW_ROWS};
use std::ops::Range;
use std::sync::Arc;

pub use edit::{RowRef, TableEdits, Undid};
pub use separator::Separator;
pub use workbook::{SHEET_ROWS, Sheet, WORKBOOK_BYTES, WORKBOOK_CELLS, Workbook};

/// The most bytes of a delimited file a table indexes; a longer file shows its start. The index
/// holds an offset for every 64th record, not the file, so this is far above what a person opens.
pub const TABLE_BYTES: ByteLen = ByteLen(1024 * 1024 * 1024);

/// How many rows are looked at to size the columns.
const WIDTH_SAMPLE_ROWS: u32 = 200;

/// The bytes at the start of a file that the separator is chosen from.
const SNIFF_BYTES: usize = 32 * 1024;

/// A column's width in characters, as far as a sample of its cells says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharWidth(pub u16);

/// The narrowest and widest a column is sized to.
const WIDTH_RANGE: (u16, u16) = (4, 60);

/// How many characters of room one character takes: two for the wide scripts, one otherwise.
fn cell_width(c: char) -> usize {
    let wide = matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F300..=0x1FAFF
        | 0x20000..=0x3FFFD);
    if wide { 2 } else { 1 }
}

/// How many rows a table has, not counting the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RowCount(pub u32);

/// A row's position among the data rows, zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RowIndex(pub u32);

/// How many columns a table has: the widest row's cell count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ColumnCount(pub u32);

/// Whether a table's first row is its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HeaderMode {
    /// Guess from the rows.
    #[default]
    Detect,
    /// The first row names the columns.
    Present,
    /// Every row is data.
    Absent,
}

/// A table. A delimited file read with [`Table::read`] is not held: its rows are parsed from the
/// file a window at a time, and a row that cannot be read (the file went away) reads as empty.
/// Rows keep the cells they were written with: a short row stays short, so [`Table::cell`] is the
/// way to read a position that may not exist.
#[derive(Debug, Clone)]
pub struct Table {
    separator: Separator,
    coverage: Coverage,
    header: Option<Vec<String>>,
    rows: Rows,
    columns: ColumnCount,
}

/// Where a table's rows are.
#[derive(Debug, Clone)]
enum Rows {
    /// In memory: a table parsed from text, or a sheet.
    Held(Vec<Vec<String>>),
    /// In a file, found by the record index.
    Indexed(Indexed),
}

impl Table {
    /// The table in `text`, cells separated by the declared `delimiter` or, when the rows say
    /// otherwise, by the separator they use (a semicolon, a pipe), and quoted the usual way
    /// (`"a,b"`, with `""` for a quote). Rows may differ in width.
    pub fn parse(text: &str, delimiter: Delimiter, mode: HeaderMode) -> Result<Self, TextError> {
        let separator = separator::sniff(text, delimiter);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(separator.byte())
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut records: Vec<Vec<String>> = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|e| TextError::Table {
                reason: e.to_string(),
            })?;
            records.push(record.iter().map(str::to_owned).collect());
        }
        Ok(Table::of_records(separator, records, mode, Coverage::Whole))
    }

    /// The table of `rows` read from a spreadsheet, `coverage` saying whether they are all of
    /// the sheet.
    pub fn from_rows(rows: Vec<Vec<String>>, mode: HeaderMode, coverage: Coverage) -> Self {
        Table::of_records(Separator::Comma, rows, mode, coverage)
    }

    fn of_records(
        separator: Separator,
        mut records: Vec<Vec<String>>,
        mode: HeaderMode,
        coverage: Coverage,
    ) -> Self {
        let columns = records.iter().map(Vec::len).max().unwrap_or(0);
        let first_is_header = match mode {
            HeaderMode::Present => !records.is_empty(),
            HeaderMode::Absent => false,
            HeaderMode::Detect => records
                .split_first()
                .is_some_and(|(first, below)| looks_like_header(first, below)),
        };
        let header = first_is_header.then(|| records.remove(0));
        Table {
            separator,
            coverage,
            header,
            rows: Rows::Held(records),
            columns: ColumnCount(u32::try_from(columns).unwrap_or(u32::MAX)),
        }
    }

    /// The delimited file `src` names, indexed by record up to [`TABLE_BYTES`] of it; what is left
    /// out shows in [`Table::coverage`]. One pass over the bytes finds the record count, the
    /// widest row and the offset of every 64th record; the rows themselves are read when asked for.
    pub fn read(
        src: impl Into<Input>,
        delimiter: Delimiter,
        mode: HeaderMode,
    ) -> Result<Self, TextError> {
        Table::read_until(src, delimiter, mode, &Deadline::NEVER)
    }

    /// [`Table::read`], ended with [`TextError::Interrupted`] when `interrupt` says so: it is asked
    /// once per chunk of the file the index pass reads.
    pub fn read_until(
        src: impl Into<Input>,
        delimiter: Delimiter,
        mode: HeaderMode,
        interrupt: &dyn Interrupt,
    ) -> Result<Self, TextError> {
        Table::open(src, delimiter, mode, TABLE_BYTES, interrupt).map(|(table, _)| table)
    }

    /// [`Table::read_until`] indexing at most `cap` bytes, and the encoding the file is read as.
    pub(crate) fn open(
        src: impl Into<Input>,
        delimiter: Delimiter,
        mode: HeaderMode,
        cap: ByteLen,
        interrupt: &dyn Interrupt,
    ) -> Result<(Self, TextCodec), TextError> {
        if cap.0 == 0 {
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
        let end = len.min(cap.0);
        let coverage = if len > end {
            Coverage::Prefix
        } else {
            Coverage::Whole
        };
        let separator = separator::sniff(&sample(&head, detected), delimiter);
        let span = (u64::from(detected.mark), end);
        let index = RecordIndex::build(
            &*source,
            detected.codec,
            separator.byte(),
            span,
            coverage,
            interrupt,
        )?;
        let columns = ColumnCount(index.widest);
        let indexed = Indexed::new(source, detected.codec, separator.byte(), index);
        let (header, indexed) = with_header(indexed, mode)?;
        let table = Table {
            separator,
            coverage,
            header,
            rows: Rows::Indexed(indexed),
            columns,
        };
        Ok((table, detected.codec))
    }

    /// Whether the table holds every row of its source or only the start.
    pub fn coverage(&self) -> Coverage {
        self.coverage
    }

    /// The separator the cells were split by.
    pub fn separator(&self) -> Separator {
        self.separator
    }

    /// How wide each column wants to be, in characters: the longest cell among the header and a
    /// sample of the first rows, held between 4 and 60 characters. A wide character (CJK, full-width)
    /// counts for two.
    pub fn column_widths(&self) -> Vec<CharWidth> {
        let (least, most) = WIDTH_RANGE;
        let sample = self
            .rows(RowIndex(0)..RowIndex(WIDTH_SAMPLE_ROWS))
            .unwrap_or_default();
        (0..self.columns.0 as usize) // a u32 fits a usize
            .map(|column| {
                let chars = |cell: &String| cell.chars().map(cell_width).sum::<usize>();
                let widest = self
                    .header
                    .iter()
                    .chain(sample.iter())
                    .filter_map(|row| row.get(column))
                    .map(chars)
                    .max()
                    .unwrap_or(0);
                CharWidth(u16::try_from(widest).unwrap_or(most).clamp(least, most))
            })
            .collect()
    }

    /// The table in a file's bytes, held whole: the encoding is detected, a byte-order mark
    /// skipped.
    pub fn parse_bytes(
        bytes: &[u8],
        delimiter: Delimiter,
        mode: HeaderMode,
    ) -> Result<Self, TextError> {
        let detected = detect(bytes, Coverage::Whole);
        let body = bytes.get(usize::from(detected.mark)..).unwrap_or_default();
        Table::parse(&TextCodec::decode(detected.codec, body), delimiter, mode)
    }

    /// The column names, when the table has a header.
    pub fn header(&self) -> Option<&[String]> {
        self.header.as_deref()
    }

    /// The number of data rows.
    pub fn row_count(&self) -> RowCount {
        match &self.rows {
            Rows::Held(rows) => RowCount(u32::try_from(rows.len()).unwrap_or(u32::MAX)),
            Rows::Indexed(indexed) => RowCount(indexed.total()),
        }
    }

    /// The width of the widest row, header included.
    pub fn columns(&self) -> ColumnCount {
        self.columns
    }

    /// The data rows in `range`, cut to the rows that exist. A file's rows are parsed here, a
    /// window at a time, and the last windows are kept for the next ask.
    pub fn rows(&self, range: Range<RowIndex>) -> Result<Vec<Vec<String>>, TextError> {
        match &self.rows {
            Rows::Held(rows) => {
                let end = (range.end.0 as usize).min(rows.len()); // a u32 fits a usize
                let start = (range.start.0 as usize).min(end);
                Ok(rows.get(start..end).unwrap_or_default().to_vec())
            }
            Rows::Indexed(indexed) => indexed.rows(range.start.0..range.end.0),
        }
    }

    /// The cell at `row` and `column`, or an empty string where the row is shorter, is missing or
    /// cannot be read.
    pub fn cell(&self, row: RowIndex, column: usize) -> String {
        match &self.rows {
            Rows::Held(rows) => rows
                .get(row.0 as usize) // a u32 fits a usize
                .and_then(|cells| cells.get(column))
                .cloned()
                .unwrap_or_default(),
            Rows::Indexed(indexed) => indexed.cell(row.0, column),
        }
    }

    /// The cells of one column, top to bottom, an empty string where a row is shorter. A file's
    /// rows are read as the iterator goes, a window at a time, so a column of a table of any size
    /// costs one window; a caller that wants to keep something of every row (a sort's key) keeps
    /// only that.
    pub fn column(&self, column: usize) -> impl Iterator<Item = String> + '_ {
        let total = self.row_count().0;
        (0..total)
            .step_by(WINDOW_ROWS as usize) // a u32 fits a usize
            .flat_map(move |from| {
                let to = from.saturating_add(WINDOW_ROWS).min(total);
                match self.rows(RowIndex(from)..RowIndex(to)) {
                    Ok(rows) => rows
                        .into_iter()
                        .map(|mut row| {
                            if column < row.len() {
                                row.swap_remove(column)
                            } else {
                                String::new()
                            }
                        })
                        .collect::<Vec<String>>(),
                    Err(_) => vec![String::new(); (to - from) as usize], // a u32 fits a usize
                }
            })
    }
}

/// The start of a file decoded, to choose its separator from.
fn sample(head: &[u8], detected: Detected) -> String {
    let body = head.get(usize::from(detected.mark)..).unwrap_or_default();
    let take = body.len().min(SNIFF_BYTES);
    let take = take - take % detected.codec.unit();
    let text = detected.codec.decode(body.get(..take).unwrap_or_default());
    text.into_owned()
}

/// `indexed` with its header taken off the rows, when the first record is one.
fn with_header(
    indexed: Indexed,
    mode: HeaderMode,
) -> Result<(Option<Vec<String>>, Indexed), TextError> {
    if indexed.records() == 0 {
        return Ok((None, indexed));
    }
    let first = indexed.window(0)?;
    let is_header = match mode {
        HeaderMode::Present => true,
        HeaderMode::Absent => false,
        HeaderMode::Detect => first
            .split_first()
            .is_some_and(|(top, below)| looks_like_header(top, below)),
    };
    match first.first() {
        Some(top) if is_header => Ok((Some(top.clone()), indexed.with_skip(1))),
        Some(_) | None => Ok((None, indexed)),
    }
}
