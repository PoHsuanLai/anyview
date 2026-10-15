//! A table file opened for showing: its sheets as tables, each with the column widths its first rows
//! ask for, and the rows of the Info tab. A delimited file is indexed, not held; a workbook's sheets
//! are held.

use super::kind::{ColumnKind, column_kind};
use crate::io::OpenError;
use anyview_archive::office_look_until;
use anyview_core::work::Stop;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, OfficeFormat, Sniffed, Source,
};
use anyview_fs::OnDisk;
use anyview_text::{CharWidth, Coverage, HeaderMode, RowIndex, Table, TableEdits, Workbook};
use ds_core::word::Word;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// One sheet: its name, its rows and how wide its columns want to be.
#[derive(Debug)]
pub struct SheetDoc {
    /// The sheet's tab name; empty for a delimited file, which has one unnamed sheet.
    pub name: String,
    /// The rows.
    pub table: Table,
    /// The width each column wants, in characters.
    pub widths: Vec<CharWidth>,
    /// What each column holds, so its cells are drawn by it (numbers sit at the trailing edge).
    pub kinds: Vec<ColumnKind>,
}

/// An opened table file.
#[derive(Debug)]
pub struct TableDoc {
    /// The sheets, in file order; never empty.
    pub sheets: Vec<Arc<SheetDoc>>,
    /// The rows of the Info tab.
    pub facts: Facts,
    /// The edits made to the cells of a delimited file, held beside the document so the view that
    /// draws it and the save that writes it see the same ones. `None` for a file that cannot be
    /// written back (a workbook, a file that is not UTF-8, one too large to be edited).
    edits: Mutex<Option<TableEdits>>,
}

/// The most bytes of a delimited file that is edited: saving builds the new file in memory.
const EDIT_BYTES: u64 = 512 * 1024 * 1024;

impl TableDoc {
    /// A document of `sheets` whose first can be edited when `edits` says so.
    pub(super) fn new(sheets: Vec<Arc<SheetDoc>>, facts: Facts, edits: Option<TableEdits>) -> Self {
        TableDoc {
            sheets,
            facts,
            edits: Mutex::new(edits),
        }
    }

    /// The edits, when the file can be edited.
    pub(crate) fn edits(&self) -> MutexGuard<'_, Option<TableEdits>> {
        self.edits.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How many rows the sheet at `at` shows now: with rows added or taken away, the edited count.
    pub(crate) fn rows_now(&self, at: usize) -> u32 {
        match (at, self.edits().as_ref()) {
            (0, Some(edits)) => edits.rows(),
            (_, Some(_) | None) => self
                .sheets
                .get(at)
                .map_or(0, |sheet| sheet.table.row_count().0),
        }
    }

    /// Whether the cells can be edited.
    pub(crate) fn is_editable(&self) -> bool {
        self.edits().is_some()
    }

    /// The file's bytes with `edits` made to them. Blocking: the file is read through once.
    pub(crate) fn written(&self, edits: &TableEdits) -> Result<Vec<u8>, anyview_text::TextError> {
        match self.sheets.first() {
            Some(sheet) => sheet.table.write_edited(edits),
            None => Err(anyview_text::TextError::NoSuchNode),
        }
    }
}

/// How many rows at the top of a table decide what a column holds.
const NUMERIC_SAMPLE_ROWS: u32 = 10_000;

/// What each column of `table` holds, as far as its first [`NUMERIC_SAMPLE_ROWS`] rows say: a table
/// of millions of rows is read for this once, in windows, and only that much of it.
fn column_kinds(table: &Table) -> Vec<ColumnKind> {
    let sample = table
        .rows(RowIndex(0)..RowIndex(NUMERIC_SAMPLE_ROWS))
        .unwrap_or_default();
    (0..table.columns().0 as usize) // a u32 fits a usize
        .map(|column| {
            column_kind(
                sample
                    .iter()
                    .map(|row| row.get(column).map_or("", String::as_str)),
            )
        })
        .collect()
}

fn sheet(name: String, table: Table) -> Arc<SheetDoc> {
    Arc::new(SheetDoc {
        widths: table.column_widths(),
        kinds: column_kinds(&table),
        name,
        table,
    })
}

fn base_facts(src: &Source, sniffed: &Sniffed) -> Facts {
    Facts::empty()
        .with(FactLabel::Kind, crate::families::kind_row(src, sniffed))
        .with(FactLabel::Size, FactValue::size(src.stamp().len))
}

fn count_text(table: &Table) -> String {
    match table.coverage() {
        Coverage::Whole => table.row_count().0.to_string(),
        Coverage::Prefix => format!("{}+", table.row_count().0),
    }
}

/// Open the table file `src`. Blocking: indexes a delimited file (one pass, up to the text crate's
/// cap) and reads a workbook whole (up to its caps on rows and cells).
pub(super) fn open(src: &Source, sniffed: &Sniffed, stop: &Stop) -> Result<TableDoc, OpenError> {
    if sniffed.kind() != FormatKind::Table {
        return Err(OpenError::Unrecognised);
    }
    match sniffed.detail() {
        FormatDetail::Table(delimiter) => {
            let table = Table::read_until(src.on_disk(), *delimiter, HeaderMode::Detect, stop)?;
            let edits = (table.can_write() && src.stamp().len.0 <= EDIT_BYTES)
                .then(|| TableEdits::new(table.row_count().0, table.columns().0));
            let facts = base_facts(src, sniffed)
                .with(FactLabel::Rows, FactValue::text(count_text(&table)))
                .with(
                    FactLabel::Columns,
                    FactValue::text(table.columns().0.to_string()),
                );
            Ok(TableDoc::new(
                vec![sheet(String::new(), table)],
                facts,
                edits,
            ))
        }
        FormatDetail::Office(format) => workbook(src, sniffed, *format, stop),
        FormatDetail::None
        | FormatDetail::Raster(_)
        | FormatDetail::Code(_)
        | FormatDetail::Tree(_)
        | FormatDetail::Text(_)
        | FormatDetail::Media(_)
        | FormatDetail::Font(_)
        | FormatDetail::Archive(_)
        | FormatDetail::Book(_) => Err(OpenError::Unrecognised),
    }
}

fn workbook(
    src: &Source,
    sniffed: &Sniffed,
    format: OfficeFormat,
    stop: &Stop,
) -> Result<TableDoc, OpenError> {
    let book = Workbook::open_until(src.on_disk(), stop)?;
    let sheets: Vec<Arc<SheetDoc>> = book
        .sheets()
        .iter()
        .map(|each| sheet(each.name.clone(), each.table.clone()))
        .collect();
    if sheets.is_empty() {
        return Err(OpenError::Text(anyview_text::TextError::Workbook {
            reason: "the workbook has no sheets".to_owned(),
        }));
    }
    // The package's own title and author are a nicety: a workbook that has none, or that is not
    // a package (the binary format), still shows its sheets.
    let look = office_look_until(src.on_disk(), format, stop).unwrap_or_default();
    let facts = look.facts().rows().iter().fold(
        base_facts(src, sniffed).with(FactLabel::Sheets, FactValue::text(sheets.len().to_string())),
        |facts, row| {
            if row.label == FactLabel::Sheets {
                facts
            } else {
                facts.with_fact(row.clone())
            }
        },
    );
    let first = &sheets[0];
    let facts = facts
        .with(
            FactLabel::Rows,
            FactValue::text(format!(
                "{} in {}",
                count_text(&first.table),
                sheet_name(first)
            )),
        )
        .with(
            FactLabel::Columns,
            FactValue::text(first.table.columns().0.to_string()),
        );
    Ok(TableDoc::new(sheets, facts, None))
}

fn sheet_name(sheet: &SheetDoc) -> &str {
    if sheet.name.is_empty() {
        FactLabel::Rows.label()
    } else {
        &sheet.name
    }
}
