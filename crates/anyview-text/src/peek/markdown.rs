//! The peek of a Markdown document: its start rendered, with the outline of what was rendered.

use super::head::{expect_kind, read_head};
use super::tally::Tally;
use crate::encoding::TextCodec;
use crate::error::TextError;
use crate::lines::split;
use crate::markdown::{Heading, NoFiles, RenderEnv, render};
use anyview_core::{FactLabel, FactValue, Facts, FormatKind, Peek, PeekBudget, Sniffed, Source};

/// What a peek of a Markdown file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownPeeked {
    /// The start of the document as HTML. Local images are shown as their alt text: a peek has no
    /// file reader.
    pub html: String,
    /// The headings of the part that was rendered.
    pub outline: Vec<Heading>,
    /// How many lines the file has, as far as the budget let the peek see.
    pub total: Tally,
    /// How the bytes were decoded.
    pub encoding: TextCodec,
}

/// The peek of the kind `Markdown`.
#[derive(Debug, Clone, Copy)]
pub struct MarkdownPeek;

impl Peek for MarkdownPeek {
    const KIND: FormatKind = FormatKind::Markdown;
    type Peeked = MarkdownPeeked;
    type Error = TextError;

    fn peek(
        src: &Source,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<MarkdownPeeked, TextError> {
        expect_kind(sniffed, Self::KIND)?;
        let head = read_head(src, budget)?;
        let rendered = render(
            &head.text,
            &RenderEnv {
                base: None,
                files: &NoFiles,
                highlighter: None,
            },
        );
        Ok(MarkdownPeeked {
            html: rendered.html,
            outline: rendered.outline,
            total: Tally::of(split(&head.text).len(), head.coverage),
            encoding: head.codec,
        })
    }

    fn facts(peeked: &MarkdownPeeked) -> Facts {
        let facts = Facts::empty().with(FactLabel::Kind, FactValue::text("Markdown document"));
        let facts = match peeked.outline.first() {
            Some(heading) => facts.with(FactLabel::Title, FactValue::text(heading.text.clone())),
            None => facts,
        };
        facts
            .with(FactLabel::Lines, FactValue::text(peeked.total.text()))
            .with(
                FactLabel::Encoding,
                FactValue::text(peeked.encoding.label()),
            )
    }
}
