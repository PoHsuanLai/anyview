//! The peek of source code: its first lines highlighted into token classes.

use super::head::{PEEK_LINES, expect_kind, read_head};
use super::tally::Tally;
use crate::code::{Highlighter, TokenLine};
use crate::encoding::TextCodec;
use crate::error::TextError;
use crate::lines::split_start;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, Sniffed, Source,
    SyntaxName,
};

/// What a peek of a source file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodePeeked {
    /// The first lines, highlighted. A language the highlighter does not have comes back as plain
    /// spans.
    pub lines: Vec<TokenLine>,
    /// How many lines the file has, as far as the budget let the peek see.
    pub total: Tally,
    /// How the bytes were decoded.
    pub encoding: TextCodec,
    /// The language the file was highlighted as.
    pub syntax: SyntaxName,
}

/// The peek of the kind `Code`. It builds the highlighter's syntax set on every call, because a
/// peek has nowhere to keep one (the trait takes no state); the set loads its syntaxes lazily, so
/// this costs only the syntax the file uses.
#[derive(Debug, Clone, Copy)]
pub struct CodePeek;

/// `Rust source`: the language name with its first letter capitalised.
fn kind_text(syntax: &SyntaxName) -> String {
    let name = syntax.as_str();
    let mut chars = name.chars();
    let first: String = chars
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    format!("{first}{} source", chars.as_str())
}

impl Peek for CodePeek {
    const KIND: FormatKind = FormatKind::Code;
    type Peeked = CodePeeked;
    type Error = TextError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<CodePeeked, TextError> {
        expect_kind(sniffed, Self::KIND)?;
        let FormatDetail::Code(syntax) = sniffed.detail() else {
            return Err(TextError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        let head = read_head(src, budget)?;
        let (start, count) = split_start(&head.text, PEEK_LINES);
        let total = Tally::of(count, head.coverage);
        let shown = start.join("\n");
        let highlighter = Highlighter::new();
        let lines = highlighter.snippet(highlighter.syntax(syntax), &shown);
        Ok(CodePeeked {
            lines,
            total,
            encoding: head.codec,
            syntax: syntax.clone(),
        })
    }

    fn facts(peeked: &CodePeeked) -> Facts {
        Facts::empty()
            .with(FactLabel::Kind, FactValue::text(kind_text(&peeked.syntax)))
            .with(FactLabel::Lines, FactValue::text(peeked.total.text()))
            .with(
                FactLabel::Encoding,
                FactValue::text(peeked.encoding.label()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_names_the_language() {
        const CASES: &[(&str, &str)] = &[
            ("rust", "Rust source"),
            ("c++", "C++ source"),
            ("c#", "C# source"),
            ("javascript", "Javascript source"),
        ];
        for (syntax, want) in CASES {
            assert_eq!(
                kind_text(&SyntaxName::new(syntax).unwrap()),
                *want,
                "{syntax}"
            );
        }
    }
}
