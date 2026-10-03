//! CSV and TSV as a table: rows that can be windowed, a header the viewer guesses or is told.

mod header;

#[cfg(test)]
mod tests;

use crate::encoding::{Coverage, TextCodec, detect};
use crate::error::TextError;
use anyview_core::Delimiter;
use header::looks_like_header;
use std::ops::Range;

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

/// A parsed table. Rows keep the cells they were written with: a short row stays short, so
/// [`Table::cell`] is the way to read a position that may not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    delimiter: Delimiter,
    header: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
    columns: ColumnCount,
}

fn byte_of(delimiter: Delimiter) -> u8 {
    match delimiter {
        Delimiter::Comma => b',',
        Delimiter::Tab => b'\t',
    }
}

impl Table {
    /// The table in `text`, cells separated by `delimiter` and quoted the usual way (`"a,b"`, with
    /// `""` for a quote). Rows may differ in width.
    pub fn parse(text: &str, delimiter: Delimiter, mode: HeaderMode) -> Result<Self, TextError> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(byte_of(delimiter))
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
        let columns = records.iter().map(Vec::len).max().unwrap_or(0);
        let first_is_header = match mode {
            HeaderMode::Present => !records.is_empty(),
            HeaderMode::Absent => false,
            HeaderMode::Detect => records
                .split_first()
                .is_some_and(|(first, below)| looks_like_header(first, below)),
        };
        let header = first_is_header.then(|| records.remove(0));
        Ok(Table {
            delimiter,
            header,
            rows: records,
            columns: ColumnCount(u32::try_from(columns).unwrap_or(u32::MAX)),
        })
    }

    /// The table in a file's bytes: the encoding is detected, a byte-order mark skipped.
    pub fn parse_bytes(
        bytes: &[u8],
        delimiter: Delimiter,
        mode: HeaderMode,
    ) -> Result<Self, TextError> {
        let detected = detect(bytes, Coverage::Whole);
        let body = bytes.get(usize::from(detected.mark)..).unwrap_or_default();
        Table::parse(&TextCodec::decode(detected.codec, body), delimiter, mode)
    }

    /// The delimiter the table was read with.
    pub fn delimiter(&self) -> Delimiter {
        self.delimiter
    }

    /// The column names, when the table has a header.
    pub fn header(&self) -> Option<&[String]> {
        self.header.as_deref()
    }

    /// The number of data rows.
    pub fn row_count(&self) -> RowCount {
        RowCount(u32::try_from(self.rows.len()).unwrap_or(u32::MAX))
    }

    /// The width of the widest row, header included.
    pub fn columns(&self) -> ColumnCount {
        self.columns
    }

    /// The data rows in `range`, cut to the rows that exist.
    pub fn rows(&self, range: Range<RowIndex>) -> &[Vec<String>] {
        let end = (range.end.0 as usize).min(self.rows.len()); // a u32 fits a usize
        let start = (range.start.0 as usize).min(end);
        &self.rows[start..end]
    }

    /// The cell at `row` and `column`, or an empty string where the row is shorter.
    pub fn cell(&self, row: RowIndex, column: usize) -> &str {
        self.rows
            .get(row.0 as usize)
            .and_then(|cells| cells.get(column))
            .map_or("", String::as_str)
    }
}
