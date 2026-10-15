//! Spreadsheets (XLSX, ODS, XLS) as tables, one per sheet.

use super::{HeaderMode, Table};
use crate::bytes::read_range;
use crate::encoding::Coverage;
use crate::error::TextError;
use anyview_archive::{ArchiveError, EntryKind, EntryLimit, ExtractLimits};
use anyview_core::work::Interrupt;
use anyview_core::{ArchiveFormat, ByteLen, Deadline, Input};
use calamine::{Cell, Data, DataRef, Reader, Sheets, open_workbook_auto_from_rs};
use std::io::Cursor;
use std::sync::Arc;

/// The largest workbook file that is opened: a sheet is read into memory whole.
pub const WORKBOOK_BYTES: ByteLen = ByteLen(128 * 1024 * 1024);

/// The largest part of a zipped workbook (shared strings, styles, the workbook's own list, an ODS's
/// content) that the reader unpacks whole, as it is stated in the zip.
const PART_BYTES: ByteLen = ByteLen(64 * 1024 * 1024);

/// The most those parts add up to.
const PARTS_BYTES: u64 = 256 * 1024 * 1024;

/// The most of a zip's central directory that is read to check its parts.
const DIRECTORY_BYTES: ByteLen = ByteLen(16 * 1024 * 1024);

/// The last serial Excel itself writes as a date (9999-12-31).
const LAST_SERIAL: f64 = 2_958_465.0;

/// The most rows kept of one sheet.
pub const SHEET_ROWS: usize = 100_000;

/// The most cells kept across a workbook's sheets.
pub const WORKBOOK_CELLS: usize = 4_000_000;

/// The most cells kept of a workbook that is only glanced at: its first sheet's start.
const START_CELLS: usize = 1_000_000;

/// How many cells of a streamed sheet are read between two asks of the interrupt.
const ASK_EVERY: usize = 4096;

/// One sheet of a workbook.
#[derive(Debug, Clone)]
pub struct Sheet {
    /// The name its tab carries.
    pub name: String,
    /// Its cells as rows.
    pub table: Table,
}

/// A workbook's sheets, in the order the file lists them.
#[derive(Debug, Clone)]
pub struct Workbook {
    sheets: Vec<Sheet>,
    sheet_count: u32,
}

fn failed(error: impl std::fmt::Display) -> TextError {
    TextError::Workbook {
        reason: error.to_string(),
    }
}

/// Refuses a zipped workbook (XLSX, XLSB, ODS) whose parts the readers unpack whole are larger
/// than they should be, or whose shared-string table states more strings than it has bytes: the
/// reader reserves for that count before it reads a string. The sheets themselves are streamed
/// and bounded by [`gather`]. Sizes are the ones the zip states.
fn check_parts(src: &Input, interrupt: &dyn Interrupt) -> Result<(), TextError> {
    let too_large = || TextError::WorkbookTooLarge {
        allowed: PART_BYTES,
    };
    let listing = anyview_archive::list_until(
        src.clone(),
        ArchiveFormat::Zip,
        EntryLimit(u32::MAX),
        DIRECTORY_BYTES,
        interrupt,
    )
    .map_err(archive_failed)?;
    if matches!(listing.count, anyview_archive::EntryCount::AtLeast(_)) {
        return Err(too_large());
    }
    let mut total = 0_u64;
    for entry in &listing.entries {
        let path = entry.path.to_ascii_lowercase();
        let unpacked_whole = entry.kind == EntryKind::File
            && !path.contains("/worksheets/")
            && [".xml", ".bin", ".rels"]
                .iter()
                .any(|end| path.ends_with(end));
        let size = entry.size.map_or(0, |size| size.0);
        if unpacked_whole {
            total = total.saturating_add(size);
            if size > PART_BYTES.0 || total > PARTS_BYTES {
                return Err(too_large());
            }
        }
    }
    let strings = anyview_archive::extract_until(
        src.clone(),
        ArchiveFormat::Zip,
        "xl/sharedStrings.xml",
        ExtractLimits::new(PART_BYTES, PART_BYTES),
        interrupt,
    );
    match strings {
        Ok(part) if states_more_than_fits(&part) => Err(failed("the shared strings are damaged")),
        Ok(_) | Err(ArchiveError::NoSuchEntry { .. } | ArchiveError::NotAFile) => Ok(()),
        Err(error) => Err(archive_failed(error)),
    }
}

fn archive_failed(error: ArchiveError) -> TextError {
    match error {
        ArchiveError::Interrupted => TextError::Interrupted,
        ArchiveError::TooLarge { allowed } | ArchiveError::OverBudget { allowed } => {
            TextError::WorkbookTooLarge { allowed }
        }
        other @ (ArchiveError::Read { .. }
        | ArchiveError::WrongKind { .. }
        | ArchiveError::NoBudget
        | ArchiveError::Malformed { .. }
        | ArchiveError::Encrypted
        | ArchiveError::NoSuchEntry { .. }
        | ArchiveError::NotAFile) => failed(other),
    }
}

/// Whether any `uniqueCount` attribute in the shared strings states more strings than the part
/// has bytes. Every occurrence is looked at, not only the one in the `sst` tag, so text placed
/// ahead of the tag cannot hide it.
fn states_more_than_fits(part: &[u8]) -> bool {
    const NAME: &[u8] = b"uniqueCount";
    let mut from = 0;
    while let Some(found) = part[from..]
        .windows(NAME.len())
        .position(|window| window == NAME)
    {
        let mut rest = &part[from + found + NAME.len()..];
        from += found + NAME.len();
        rest = rest.trim_ascii_start();
        let [b'=', tail @ ..] = rest else { continue };
        rest = tail.trim_ascii_start();
        let [b'"' | b'\'', tail @ ..] = rest else {
            continue;
        };
        let digits = tail.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let count = std::str::from_utf8(&tail[..digits])
            .ok()
            .and_then(|text| text.parse::<u64>().ok());
        if count.is_none_or(|count| count > part.len() as u64) {
            return true;
        }
    }
    false
}

/// A spreadsheet's serial day number as `YYYY-MM-DD`. Serial 25569 is 1970-01-01; serials before
/// 61 are off by the day Excel counts for a 1900-02-29 that never was, which a viewer ignores.
fn civil_date(days: i64) -> String {
    let z = days - 25_569 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

/// A serial date-time as `YYYY-MM-DD` or `YYYY-MM-DD hh:mm:ss`.
fn date_text(serial: f64) -> String {
    let days = serial.floor();
    let seconds = ((serial - days) * 86_400.0).round() as i64; // a fraction of a day
    let date = civil_date(days as i64); // `cell_text` passes only serials up to `LAST_SERIAL`
    if seconds == 0 {
        date
    } else {
        format!(
            "{date} {:02}:{:02}:{:02}",
            seconds / 3_600,
            seconds / 60 % 60,
            seconds % 60
        )
    }
}

/// A cell as the text a table shows.
fn cell_text(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => text.clone(),
        Data::Int(n) => n.to_string(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{f:.0}"),
        Data::Float(f) => f.to_string(),
        Data::Bool(b) => b.to_string().to_uppercase(),
        Data::DateTime(date)
            if date.is_datetime() && (0.0..=LAST_SERIAL).contains(&date.as_f64()) =>
        {
            date_text(date.as_f64())
        }
        Data::DateTime(date) => date.as_f64().to_string(),
        Data::Error(error) => error.to_string(),
    }
}

/// A cell that is only needed as text, and where it lies.
type Placed = (u32, u32, String);

/// The rows of the cells `next` yields, which a streaming reader gives one at a time, so a sheet
/// that claims a million rows and sixteen thousand columns costs the cells it holds and not the
/// grid they span. `interrupt` is asked every [`ASK_EVERY`] cells. At most `left` cells are looked at (empty ones included, so a sheet of nothing
/// but empty cells ends too), at most [`SHEET_ROWS`] rows are kept, and the grid built from them
/// is at most `left` cells.
fn gather<'a>(
    mut next: impl FnMut() -> Result<Option<Cell<DataRef<'a>>>, TextError>,
    left: usize,
    interrupt: &dyn Interrupt,
) -> Result<(Vec<Vec<String>>, Coverage), TextError> {
    let mut cells: Vec<Placed> = Vec::new();
    let mut coverage = Coverage::Whole;
    let (mut first_row, mut first_col, mut last_col) = (u32::MAX, u32::MAX, 0);
    let mut seen = 0_usize;
    while let Some(cell) = next()? {
        seen += 1;
        if seen.is_multiple_of(ASK_EVERY) && interrupt.interrupted() {
            return Err(TextError::Interrupted);
        }
        let (row, column) = cell.get_position();
        first_row = first_row.min(row);
        if seen > left || row - first_row >= SHEET_ROWS as u32 {
            coverage = Coverage::Prefix;
            break;
        }
        let text = cell_text(&Data::from(cell.get_value().clone()));
        if text.is_empty() {
            continue;
        }
        first_col = first_col.min(column);
        last_col = last_col.max(column);
        cells.push((row, column, text));
    }
    let Some(last_row) = cells.iter().map(|(row, _, _)| *row).max() else {
        return Ok((Vec::new(), coverage));
    };
    let width = (last_col - first_col) as usize + 1; // a u32 fits a usize
    let height = (last_row - first_row) as usize + 1;
    let kept = height.min(SHEET_ROWS).min((left / width).max(1));
    if kept < height {
        coverage = Coverage::Prefix;
    }
    let mut rows = vec![vec![String::new(); width]; kept];
    for (row, column, text) in cells {
        if let Some(slot) = rows
            .get_mut((row - first_row) as usize) // a u32 fits a usize
            .and_then(|cells| cells.get_mut((column - first_col) as usize))
        {
            *slot = text;
        }
    }
    Ok((rows, coverage))
}

/// The rows of a sheet a reader built whole, cut to what is kept: [`SHEET_ROWS`] rows and `left`
/// cells.
fn cut(range: &calamine::Range<Data>, left: usize) -> (Vec<Vec<String>>, Coverage) {
    let width = range.width().max(1);
    let rows_allowed = SHEET_ROWS.min(left / width);
    let mut coverage = Coverage::Whole;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in range.rows() {
        if rows.len() >= rows_allowed {
            coverage = Coverage::Prefix;
            break;
        }
        rows.push(row.iter().map(cell_text).collect());
    }
    (rows, coverage)
}

/// The rows of the sheet `name`: streamed where the format allows it (XLSX, XLSB), whole where
/// the reader builds the grid itself (XLS, whose grid the format limits to 65 536 by 256 cells,
/// and ODS, whose reader bounds its repeats).
fn read_sheet(
    workbook: &mut Sheets<Cursor<Arc<[u8]>>>,
    name: &str,
    left: usize,
    interrupt: &dyn Interrupt,
) -> Result<(Vec<Vec<String>>, Coverage), TextError> {
    match workbook {
        Sheets::Xlsx(book) => {
            let mut reader = book.worksheet_cells_reader(name).map_err(failed)?;
            gather(|| reader.next_cell().map_err(failed), left, interrupt)
        }
        Sheets::Xlsb(book) => {
            let mut reader = book.worksheet_cells_reader(name).map_err(failed)?;
            gather(|| reader.next_cell().map_err(failed), left, interrupt)
        }
        other @ (Sheets::Xls(_) | Sheets::Ods(_)) => {
            Ok(cut(&other.worksheet_range(name).map_err(failed)?, left))
        }
    }
}

impl Workbook {
    /// The workbook the file `src` names. At most [`SHEET_ROWS`] rows are kept of a sheet and
    /// [`WORKBOOK_CELLS`] cells of the whole; what is cut is a sheet whose table says it was
    /// only a start. A file over [`WORKBOOK_BYTES`] is refused.
    pub fn open(src: impl Into<Input>) -> Result<Self, TextError> {
        Workbook::open_until(src, &Deadline::NEVER)
    }

    /// [`Workbook::open`], ended with [`TextError::Interrupted`] when `interrupt` says so: it is
    /// asked every few thousand cells of an XLSX or XLSB sheet, and between sheets. An XLS or an
    /// ODS is built whole by calamine, which nothing here can end.
    pub fn open_until(src: impl Into<Input>, interrupt: &dyn Interrupt) -> Result<Self, TextError> {
        Workbook::read(&src.into(), usize::MAX, WORKBOOK_CELLS, interrupt)
    }

    /// The start of the workbook the file `src` names, for a glance: its first sheet only, at
    /// most [`SHEET_ROWS`] rows and a million cells of it. [`Workbook::sheet_count`] still counts
    /// every sheet the file has.
    pub fn open_start(src: impl Into<Input>) -> Result<Self, TextError> {
        Workbook::open_start_until(src, &Deadline::NEVER)
    }

    /// [`Workbook::open_start`], ended as [`Workbook::open_until`] is.
    pub fn open_start_until(
        src: impl Into<Input>,
        interrupt: &dyn Interrupt,
    ) -> Result<Self, TextError> {
        Workbook::read(&src.into(), 1, START_CELLS, interrupt)
    }

    fn read(
        src: &Input,
        sheets: usize,
        cells: usize,
        interrupt: &dyn Interrupt,
    ) -> Result<Self, TextError> {
        if src.stamp().len.0.max(src.bytes().len().0) > WORKBOOK_BYTES.0 {
            return Err(TextError::WorkbookTooLarge {
                allowed: WORKBOOK_BYTES,
            });
        }
        // The sheet is read into memory whole anyway, so the workbook is read from the bytes
        // (sniffed by content), whether they are a file or were handed in.
        let bytes: Arc<[u8]> = read_range(src.bytes(), 0..WORKBOOK_BYTES.0)?.into();
        if bytes.starts_with(b"PK") {
            check_parts(src, interrupt)?;
        }
        // The readers index and allocate from what the file states; a panic in one is a workbook
        // that does not open.
        let mut workbook = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            open_workbook_auto_from_rs(Cursor::new(bytes))
        }))
        .map_err(|_| failed("the file is damaged"))?
        .map_err(failed)?;
        let names = workbook.sheet_names();
        let sheet_count = u32::try_from(names.len()).unwrap_or(u32::MAX);
        let mut left = cells;
        let mut kept = Vec::new();
        for name in names.into_iter().take(sheets) {
            if interrupt.interrupted() {
                return Err(TextError::Interrupted);
            }
            let (rows, coverage) = read_sheet(&mut workbook, &name, left, interrupt)?;
            left = left.saturating_sub(rows.iter().map(Vec::len).sum());
            kept.push(Sheet {
                name,
                table: Table::from_rows(rows, HeaderMode::Detect, coverage),
            });
        }
        Ok(Workbook {
            sheets: kept,
            sheet_count,
        })
    }

    /// How many sheets the file has, which is more than [`Workbook::sheets`] holds when only the
    /// start was read.
    pub fn sheet_count(&self) -> u32 {
        self.sheet_count
    }

    /// The sheets, in file order.
    pub fn sheets(&self) -> &[Sheet] {
        &self.sheets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_dates_read_as_calendar_dates() {
        // name, serial, text
        const CASES: &[(&str, f64, &str)] = &[
            ("a date after the leap-day bug", 61.0, "1900-03-01"),
            ("new year 2000", 36526.0, "2000-01-01"),
            ("leap day 2024", 45351.0, "2024-02-29"),
            ("unix epoch", 25569.0, "1970-01-01"),
            ("with a time", 45351.5, "2024-02-29 12:00:00"),
        ];
        for (name, serial, want) in CASES {
            assert_eq!(date_text(*serial), *want, "{name}");
        }
    }

    #[test]
    fn a_string_table_that_states_more_strings_than_it_has_bytes_is_refused() {
        // name, shared strings, refused
        const CASES: &[(&str, &str, bool)] = &[
            (
                "a real count",
                r#"<sst count="2" uniqueCount="2"><si/><si/></sst>"#,
                false,
            ),
            (
                "a count of the whole planet",
                r#"<sst uniqueCount="100000000000"/>"#,
                true,
            ),
            (
                "single quotes and spaces",
                "<sst uniqueCount = '99999999'/>",
                true,
            ),
            (
                "a count too long for a number",
                r#"<sst uniqueCount="99999999999999999999999"/>"#,
                true,
            ),
            (
                "a decoy ahead of the tag",
                r#"<!-- uniqueCount="1" --><sst uniqueCount="99999999"/>"#,
                true,
            ),
            ("no count", "<sst><si/></sst>", false),
        ];
        for (name, part, refused) in CASES {
            assert_eq!(states_more_than_fits(part.as_bytes()), *refused, "{name}");
        }
    }

    #[test]
    fn a_serial_beyond_any_date_shows_as_a_number() {
        let huge = Data::DateTime(calamine::ExcelDateTime::new(
            1e300,
            calamine::ExcelDateTimeType::DateTime,
            false,
        ));
        assert_eq!(cell_text(&huge), 1e300_f64.to_string());
    }

    #[test]
    fn cells_read_as_a_person_would_write_them() {
        // name, cell, text
        let cases: Vec<(&str, Data, &str)> = vec![
            ("empty", Data::Empty, ""),
            ("a whole float", Data::Float(3.0), "3"),
            ("a fraction", Data::Float(2.5), "2.5"),
            ("an int", Data::Int(-7), "-7"),
            ("a bool", Data::Bool(true), "TRUE"),
            ("text", Data::String("hi".into()), "hi"),
        ];
        for (name, cell, want) in cases {
            assert_eq!(cell_text(&cell), want, "{name}");
        }
    }
}
