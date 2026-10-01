//! The peek of a delimited table: its first rows and its shape.

use super::head::{PEEK_LINES, expect_kind, read_head};
use super::tally::Tally;
use crate::encoding::TextCodec;
use crate::error::TextError;
use crate::table::{ColumnCount, HeaderMode, RowIndex, Table};
use anyview_core::{
    Delimiter, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, Sniffed,
    Source,
};

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
    /// What separates the cells.
    pub delimiter: Delimiter,
    /// How the bytes were decoded.
    pub encoding: TextCodec,
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
        let FormatDetail::Table(delimiter) = sniffed.detail() else {
            return Err(TextError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        let head = read_head(src, budget)?;
        let table = Table::parse(&head.text, *delimiter, HeaderMode::Detect)?;
        let rows = table
            .rows(RowIndex(0)..RowIndex(PEEK_LINES as u32)) // 40
            .to_vec();
        Ok(TablePeeked {
            header: table.header().map(<[String]>::to_vec),
            rows,
            total_rows: Tally::of(table.row_count().0 as usize, head.coverage), // a u32 fits a usize
            columns: table.columns(),
            delimiter: *delimiter,
            encoding: head.codec,
        })
    }

    fn facts(peeked: &TablePeeked) -> Facts {
        let kind = match peeked.delimiter {
            Delimiter::Comma => "CSV table",
            Delimiter::Tab => "TSV table",
        };
        Facts::empty()
            .with(FactLabel::Kind, FactValue::text(kind))
            .with(FactLabel::Rows, FactValue::text(peeked.total_rows.text()))
            .with(
                FactLabel::Columns,
                FactValue::text(peeked.columns.0.to_string()),
            )
            .with(
                FactLabel::Encoding,
                FactValue::text(peeked.encoding.label()),
            )
    }
}
