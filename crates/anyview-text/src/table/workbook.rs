//! Spreadsheets (XLSX, ODS, XLS) as tables, one per sheet.

use super::{HeaderMode, Table};
use crate::encoding::Coverage;
use crate::error::TextError;
use anyview_core::{ByteLen, Source};
use calamine::{Data, Reader, open_workbook_auto};

/// The largest workbook file that is opened: a sheet is read into memory whole.
pub const WORKBOOK_BYTES: ByteLen = ByteLen(128 * 1024 * 1024);

/// The most rows kept of one sheet.
pub const SHEET_ROWS: usize = 100_000;

/// The most cells kept across a workbook's sheets.
pub const WORKBOOK_CELLS: usize = 4_000_000;

/// One sheet of a workbook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The name its tab carries.
    pub name: String,
    /// Its cells as rows.
    pub table: Table,
}

/// A workbook's sheets, in the order the file lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workbook {
    sheets: Vec<Sheet>,
}

fn failed(error: impl std::fmt::Display) -> TextError {
    TextError::Workbook {
        reason: error.to_string(),
    }
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
    let date = civil_date(days as i64); // a spreadsheet's days fit an i64
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
        Data::DateTime(date) if date.is_datetime() => date_text(date.as_f64()),
        Data::DateTime(date) => date.as_f64().to_string(),
        Data::Error(error) => error.to_string(),
    }
}

impl Workbook {
    /// The workbook the file `src` names. At most [`SHEET_ROWS`] rows are kept of a sheet and
    /// [`WORKBOOK_CELLS`] cells of the whole; what is cut is a sheet whose table says it was
    /// only a start. A file over [`WORKBOOK_BYTES`] is refused.
    pub fn open(src: &Source) -> Result<Self, TextError> {
        if src.stamp().len.0 > WORKBOOK_BYTES.0 {
            return Err(TextError::WorkbookTooLarge {
                allowed: WORKBOOK_BYTES,
            });
        }
        let mut workbook = open_workbook_auto(src.path().as_path()).map_err(failed)?;
        let mut left = WORKBOOK_CELLS;
        let mut sheets = Vec::new();
        for name in workbook.sheet_names() {
            let range = workbook.worksheet_range(&name).map_err(failed)?;
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
            left = left.saturating_sub(rows.len() * width);
            sheets.push(Sheet {
                name,
                table: Table::from_rows(rows, HeaderMode::Detect, coverage),
            });
        }
        Ok(Workbook { sheets })
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
