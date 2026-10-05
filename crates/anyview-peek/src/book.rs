//! The peek of a book: what its package says and its cover. An EPUB's cover is the image its
//! package names; a comic's is its first page. Neither is decoded beyond that one image, and the
//! reading order is not walked.

use crate::body::Body;
use crate::described::Described;
use crate::error::PeekError;
use crate::frames::cover_picture;
use anyview_book::{BookError, Comic, Epub, comic_cover, epub_cover};
use anyview_core::{
    BookFormat, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, Sniffed,
    Source,
};
use anyview_image::ImagePeek;
use std::sync::Arc;

/// What a peek of a book holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookLook {
    /// The file's type in words, for a book with no cover.
    pub described: Described,
    /// The rows the book gave: title, author, publisher, language and its length.
    pub rows: Facts,
    /// The cover, reduced to the budget.
    pub cover: Option<Arc<ImagePeek>>,
}

/// The peek of the kind `Book`.
#[derive(Debug, Clone, Copy)]
pub struct BookPeek;

fn row(facts: Facts, label: FactLabel, text: Option<&String>) -> Facts {
    match text {
        Some(text) => facts.with(label, FactValue::text(text.clone())),
        None => facts,
    }
}

fn epub_look(src: &Source, budget: &PeekBudget) -> Result<(Facts, Option<ImagePeek>), BookError> {
    let epub = Epub::open(src.path())?;
    let meta = epub.meta();
    let facts = row(Facts::empty(), FactLabel::Title, meta.title.as_ref());
    let facts = row(facts, FactLabel::Author, meta.author.as_ref());
    let facts = row(facts, FactLabel::Publisher, meta.publisher.as_ref());
    let facts = row(facts, FactLabel::Language, meta.language.as_ref());
    let facts = facts.with(
        FactLabel::Chapters,
        FactValue::text(epub.sections().get().to_string()),
    );
    let cover =
        epub_cover(&epub).and_then(|cover| cover_picture(&cover.bytes, &cover.name, budget));
    Ok((facts, cover))
}

fn comic_look(src: &Source, budget: &PeekBudget) -> Result<(Facts, Option<ImagePeek>), BookError> {
    let comic = Comic::open(src.path())?;
    let facts = Facts::empty().with(
        FactLabel::Pages,
        FactValue::text(comic.sections().get().to_string()),
    );
    let cover =
        comic_cover(src.path()).and_then(|cover| cover_picture(&cover.bytes, &cover.name, budget));
    Ok((facts, cover))
}

impl Peek for BookPeek {
    const KIND: FormatKind = FormatKind::Book;
    type Peeked = BookLook;
    type Error = PeekError;

    fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> Result<BookLook, PeekError> {
        let described = Described::of(sniffed);
        let FormatDetail::Book(format) = sniffed.detail() else {
            return Err(PeekError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        let looked = match format {
            BookFormat::Epub => epub_look(src, budget),
            BookFormat::Cbz => comic_look(src, budget),
        };
        // A book that cannot be opened still has its type, size and date to show.
        let (rows, cover) = looked.unwrap_or_else(|_| (Facts::empty(), None));
        Ok(BookLook {
            described,
            rows,
            cover: cover.map(Arc::new),
        })
    }

    fn facts(peeked: &BookLook) -> Facts {
        peeked.rows.rows().iter().fold(
            Facts::empty().with(
                FactLabel::Kind,
                FactValue::text(peeked.described.kind.clone()),
            ),
            |facts, row| facts.with(row.label, row.value.clone()),
        )
    }
}

impl From<BookLook> for Body {
    fn from(peeked: BookLook) -> Self {
        match peeked.cover {
            Some(cover) => Body::Picture(cover),
            None => Body::FactsOnly(peeked.described),
        }
    }
}
