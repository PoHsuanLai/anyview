//! CSV and TSV as a table: rows that can be windowed, a header the viewer guesses or is told.

mod header;
mod separator;
mod workbook;

#[cfg(test)]
mod tests;

use crate::encoding::{Coverage, TextCodec, detect};
use crate::error::TextError;
use crate::peek::head::read_limited;
use anyview_core::{ByteLen, Delimiter, Input};
use header::looks_like_header;
use std::ops::Range;

pub use separator::Separator;
pub use workbook::{SHEET_ROWS, Sheet, WORKBOOK_BYTES, WORKBOOK_CELLS, Workbook};

/// The most bytes of a delimited file a table reads.
pub const TABLE_BYTES: ByteLen = ByteLen(64 * 1024 * 1024);

/// The most rows a table keeps of a delimited file.
pub const TABLE_ROWS: usize = 200_000;

/// How many rows are looked at to size the columns.
const WIDTH_SAMPLE_ROWS: usize = 100;

/// A column's width in characters, as far as a sample of its cells says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharWidth(pub u16);

/// The narrowest and widest a column is sized to.
const WIDTH_RANGE: (u16, u16) = (4, 40);

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
    separator: Separator,
    coverage: Coverage,
    header: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
    columns: ColumnCount,
}

impl Table {
    /// The table in `text`, cells separated by the declared `delimiter` or, when the rows say
    /// otherwise, by the separator they use (a semicolon, a pipe), and quoted the usual way
    /// (`"a,b"`, with `""` for a quote). Rows may differ in width.
    pub fn parse(text: &str, delimiter: Delimiter, mode: HeaderMode) -> Result<Self, TextError> {
        Table::parse_limited(text, delimiter, mode, usize::MAX)
    }

    /// [`Table::parse`] keeping at most `max_rows` rows; a table that stopped there says it holds
    /// only the start of its text.
    fn parse_limited(
        text: &str,
        delimiter: Delimiter,
        mode: HeaderMode,
        max_rows: usize,
    ) -> Result<Self, TextError> {
        let separator = separator::sniff(text, delimiter);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(separator.byte())
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut records: Vec<Vec<String>> = Vec::new();
        let mut coverage = Coverage::Whole;
        for record in reader.records() {
            let record = record.map_err(|e| TextError::Table {
                reason: e.to_string(),
            })?;
            if records.len() >= max_rows {
                coverage = Coverage::Prefix;
                break;
            }
            records.push(record.iter().map(str::to_owned).collect());
        }
        Ok(Table::of_records(separator, records, mode, coverage))
    }

    /// The start of the table in `text`: its first `keep` data rows (the header, when there is
    /// one, comes besides them), and how many data rows the text has in all. Counting the rest
    /// holds none of them, so a file of ten million rows costs `keep`. The column count is the
    /// widest row's of all of them.
    pub(crate) fn parse_start(
        text: &str,
        delimiter: Delimiter,
        mode: HeaderMode,
        keep: usize,
    ) -> Result<(Self, RowCount), TextError> {
        let separator = separator::sniff(text, delimiter);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(separator.byte())
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut record = csv::StringRecord::new();
        let mut records: Vec<Vec<String>> = Vec::new();
        let (mut total, mut widest) = (0_usize, 0_usize);
        while reader
            .read_record(&mut record)
            .map_err(|e| TextError::Table {
                reason: e.to_string(),
            })?
        {
            total += 1;
            widest = widest.max(record.len());
            // One more than `keep`: the first record may turn out to be the header.
            if records.len() <= keep {
                records.push(record.iter().map(str::to_owned).collect());
            }
        }
        let table = Table::of_records(separator, records, mode, Coverage::Whole);
        let header = usize::from(table.header.is_some());
        let table = Table {
            columns: ColumnCount(u32::try_from(widest).unwrap_or(u32::MAX)),
            ..table
        };
        let data_rows = total - header;
        Ok((
            table,
            RowCount(u32::try_from(data_rows).unwrap_or(u32::MAX)),
        ))
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
            rows: records,
            columns: ColumnCount(u32::try_from(columns).unwrap_or(u32::MAX)),
        }
    }

    /// The delimited file `src` names, reading at most [`TABLE_BYTES`] and keeping at most
    /// [`TABLE_ROWS`] rows; what is left out shows in [`Table::coverage`].
    pub fn read(
        src: impl Into<Input>,
        delimiter: Delimiter,
        mode: HeaderMode,
    ) -> Result<Self, TextError> {
        let head = read_limited(&src.into(), TABLE_BYTES)?;
        let table = Table::parse_limited(&head.text, delimiter, mode, TABLE_ROWS)?;
        let coverage = match (head.coverage, table.coverage) {
            (Coverage::Whole, Coverage::Whole) => Coverage::Whole,
            (Coverage::Prefix, _) | (_, Coverage::Prefix) => Coverage::Prefix,
        };
        Ok(Table { coverage, ..table })
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
    /// sample of the first rows, held between 4 and 40 characters.
    pub fn column_widths(&self) -> Vec<CharWidth> {
        let (least, most) = WIDTH_RANGE;
        (0..self.columns.0 as usize) // a u32 fits a usize
            .map(|column| {
                let chars = |cell: &String| cell.chars().count();
                let widest = self
                    .header
                    .iter()
                    .chain(self.rows.iter().take(WIDTH_SAMPLE_ROWS))
                    .filter_map(|row| row.get(column))
                    .map(chars)
                    .max()
                    .unwrap_or(0);
                CharWidth(u16::try_from(widest).unwrap_or(most).clamp(least, most))
            })
            .collect()
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
