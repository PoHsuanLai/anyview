//! The peek of plain text: its first lines, how many lines there are, how it is encoded.

use super::head::{PEEK_LINES, expect_kind, read_head};
use super::tally::Tally;
use crate::encoding::TextCodec;
use crate::error::TextError;
use crate::lines::split_start;
use anyview_core::{FactLabel, FactValue, Facts, FormatKind, Peek, PeekBudget, Sniffed, Source};

/// What a peek of a plain text file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainPeeked {
    /// The first lines.
    pub lines: Vec<String>,
    /// How many lines the file has, as far as the budget let the peek see.
    pub total: Tally,
    /// How the bytes were decoded.
    pub encoding: TextCodec,
}

/// The peek of the kind `PlainText`.
#[derive(Debug, Clone, Copy)]
pub struct PlainPeek;

impl Peek for PlainPeek {
    const KIND: FormatKind = FormatKind::PlainText;
    type Peeked = PlainPeeked;
    type Error = TextError;

    fn peek(
        src: &Source,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<PlainPeeked, TextError> {
        expect_kind(sniffed, Self::KIND)?;
        let head = read_head(src, budget)?;
        let (lines, count) = split_start(&head.text, PEEK_LINES);
        let total = Tally::of(count, head.coverage);
        Ok(PlainPeeked {
            lines,
            total,
            encoding: head.codec,
        })
    }

    fn facts(peeked: &PlainPeeked) -> Facts {
        Facts::empty()
            .with(FactLabel::Kind, FactValue::text("Plain text"))
            .with(FactLabel::Lines, FactValue::text(peeked.total.text()))
            .with(
                FactLabel::Encoding,
                FactValue::text(peeked.encoding.label()),
            )
    }
}
