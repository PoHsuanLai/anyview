//! A table file read for showing: its sheets as tables, each with the column widths its first rows
//! ask for, and the rows of the Info tab.

use crate::io::OpenError;
use anyview_archive::office_look;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, OfficeFormat, Sniffed, Source,
};
use anyview_fs::OnDisk;
use anyview_text::{CharWidth, Coverage, HeaderMode, Table, Workbook};
use ds_core::word::Word;
use std::sync::Arc;

/// One sheet: its name, its rows and how wide its columns want to be.
#[derive(Debug)]
pub struct SheetDoc {
    /// The sheet's tab name; empty for a delimited file, which has one unnamed sheet.
    pub name: String,
    /// The rows.
    pub table: Table,
    /// The width each column wants, in characters.
    pub widths: Vec<CharWidth>,
}

/// An opened table file.
#[derive(Debug)]
pub struct TableDoc {
    /// The sheets, in file order; never empty.
    pub sheets: Vec<Arc<SheetDoc>>,
    /// The rows of the Info tab.
    pub facts: Facts,
}

fn sheet(name: String, table: Table) -> Arc<SheetDoc> {
    Arc::new(SheetDoc {
        widths: table.column_widths(),
        name,
        table,
    })
}

fn base_facts(src: &Source, sniffed: &Sniffed) -> Facts {
    Facts::empty()
        .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
        .with(FactLabel::Size, FactValue::size(src.stamp().len))
}

fn count_text(table: &Table) -> String {
    match table.coverage() {
        Coverage::Whole => table.row_count().0.to_string(),
        Coverage::Prefix => format!("{}+", table.row_count().0),
    }
}

/// Open the table file `src`. Blocking: reads and parses it whole, up to the caps of the text
/// crate (a delimited file's bytes and rows, a workbook's rows and cells).
pub(super) fn open(src: &Source, sniffed: &Sniffed) -> Result<TableDoc, OpenError> {
    if sniffed.kind() != FormatKind::Table {
        return Err(OpenError::Unrecognised);
    }
    match sniffed.detail() {
        FormatDetail::Table(delimiter) => {
            let table = Table::read(src.on_disk(), *delimiter, HeaderMode::Detect)?;
            let facts = base_facts(src, sniffed)
                .with(FactLabel::Rows, FactValue::text(count_text(&table)))
                .with(
                    FactLabel::Columns,
                    FactValue::text(table.columns().0.to_string()),
                );
            Ok(TableDoc {
                sheets: vec![sheet(String::new(), table)],
                facts,
            })
        }
        FormatDetail::Office(format) => workbook(src, sniffed, *format),
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

fn workbook(src: &Source, sniffed: &Sniffed, format: OfficeFormat) -> Result<TableDoc, OpenError> {
    let book = Workbook::open(src.on_disk())?;
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
    let look = office_look(src.on_disk(), format).unwrap_or_default();
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
    Ok(TableDoc { sheets, facts })
}

fn sheet_name(sheet: &SheetDoc) -> &str {
    if sheet.name.is_empty() {
        FactLabel::Rows.label()
    } else {
        &sheet.name
    }
}
