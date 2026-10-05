//! The peek of a delimited table: its first rows and its shape.

use super::head::{PEEK_LINES, expect_kind, read_head};
use super::tally::Tally;
use crate::encoding::TextCodec;
use crate::error::TextError;
use crate::table::{ColumnCount, HeaderMode, RowIndex, Table, Workbook};
use anyview_core::{
    Delimiter, FactLabel, FactValue, Facts, FormatDetail, FormatKind, OfficeFormat, Peek,
    PeekBudget, Sniffed, Source,
};
use ds_core::word::Word;

/// What a peek of a table holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablePeeked {
    /// The column names, when the first row looks like them.
    pub header: Option<Vec<String>>,
    /// The first rows.
    pub rows: Vec<Vec<String>>,
    /// How many data rows the table has, as far as the budget let the peek see.
    pub total_rows: Tally,
    /// The widest row's cell count among the rows seen.
    pub columns: ColumnCount,
    /// Where the rows came from.
    pub source: TableSource,
}

/// What a table peek read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableSource {
    /// A delimited text file.
    Delimited {
        /// What the file's name says separates the cells.
        delimiter: Delimiter,
        /// How the bytes were decoded.
        encoding: TextCodec,
    },
    /// The first sheet of a spreadsheet.
    Sheet {
        /// The spreadsheet's format.
        format: OfficeFormat,
        /// The name of the sheet shown.
        name: String,
        /// How many sheets the workbook has.
        sheets: u32,
    },
}

/// The peek of the kind `Table`.
#[derive(Debug, Clone, Copy)]
pub struct TablePeek;

impl Peek for TablePeek {
    const KIND: FormatKind = FormatKind::Table;
    type Peeked = TablePeeked;
    type Error = TextError;

    fn peek(
        src: &Source,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<TablePeeked, TextError> {
        expect_kind(sniffed, Self::KIND)?;
        match sniffed.detail() {
            FormatDetail::Table(delimiter) => delimited(src, *delimiter, budget),
            FormatDetail::Office(format) => sheet(src, *format, budget),
            FormatDetail::None
            | FormatDetail::Raster(_)
            | FormatDetail::Code(_)
            | FormatDetail::Tree(_)
            | FormatDetail::Text(_)
            | FormatDetail::Media(_)
            | FormatDetail::Font(_)
            | FormatDetail::Archive(_)
            | FormatDetail::Book(_) => Err(TextError::WrongKind {
                kind: sniffed.kind(),
            }),
        }
    }

    fn facts(peeked: &TablePeeked) -> Facts {
        let kind = match &peeked.source {
            TableSource::Delimited {
                delimiter: Delimiter::Comma,
                ..
            } => "CSV table".to_owned(),
            TableSource::Delimited {
                delimiter: Delimiter::Tab,
                ..
            } => "TSV table".to_owned(),
            TableSource::Sheet { format, .. } => {
                format!("{} spreadsheet", format.label().to_uppercase())
            }
        };
        let facts = Facts::empty()
            .with(FactLabel::Kind, FactValue::text(kind))
            .with(FactLabel::Rows, FactValue::text(peeked.total_rows.text()))
            .with(
                FactLabel::Columns,
                FactValue::text(peeked.columns.0.to_string()),
            );
        match &peeked.source {
            TableSource::Delimited { encoding, .. } => {
                facts.with(FactLabel::Encoding, FactValue::text(encoding.label()))
            }
            TableSource::Sheet { name, sheets, .. } => facts.with(
                FactLabel::Sheets,
                FactValue::text(format!("{sheets}, showing {name}")),
            ),
        }
    }
}

fn delimited(
    src: &Source,
    delimiter: Delimiter,
    budget: &PeekBudget,
) -> Result<TablePeeked, TextError> {
    {
        let head = read_head(src, budget)?;
        let table = Table::parse(&head.text, delimiter, HeaderMode::Detect)?;
        Ok(of_table(
            &table,
            Tally::of(table.row_count().0 as usize, head.coverage), // a u32 fits a usize
            TableSource::Delimited {
                delimiter,
                encoding: head.codec,
            },
        ))
    }
}

fn sheet(
    src: &Source,
    format: OfficeFormat,
    budget: &PeekBudget,
) -> Result<TablePeeked, TextError> {
    if src.stamp().len.0 > budget.bytes.0 {
        return Err(TextError::WorkbookTooLarge {
            allowed: budget.bytes,
        });
    }
    let workbook = Workbook::open(src)?;
    let sheets = u32::try_from(workbook.sheets().len()).unwrap_or(u32::MAX);
    let first = workbook
        .sheets()
        .first()
        .ok_or_else(|| TextError::Workbook {
            reason: "the workbook has no sheets".to_owned(),
        })?;
    let total = Tally::of(first.table.row_count().0 as usize, first.table.coverage()); // a u32 fits a usize
    Ok(of_table(
        &first.table,
        total,
        TableSource::Sheet {
            format,
            name: first.name.clone(),
            sheets,
        },
    ))
}

/// What a peek holds of `table`: its header and first rows.
fn of_table(table: &Table, total_rows: Tally, source: TableSource) -> TablePeeked {
    TablePeeked {
        header: table.header().map(<[String]>::to_vec),
        rows: table
            .rows(RowIndex(0)..RowIndex(PEEK_LINES as u32)) // 40
            .to_vec(),
        total_rows,
        columns: table.columns(),
        source,
    }
}
