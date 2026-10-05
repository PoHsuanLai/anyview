//! A book opened: an EPUB's package and contents, or a comic's page list. Reading a section
//! blocks (it unpacks a chapter, or a page, from the zip), so it runs on a worker (`Job::Section`);
//! the window holds only the page a worker returned.

use crate::io::{OpenError, OpenLink};
use anyview_book::{BookError, Comic, Epub, TocEntry};
use anyview_core::{
    BookFormat, FactLabel, FactValue, Facts, FormatDetail, SectionCount, SectionIndex, Sniffed,
    Source,
};

/// What a book is made of.
#[derive(Debug)]
enum Shape {
    Epub { epub: Epub, contents: Vec<TocEntry> },
    Comic(Comic),
}

/// An opened EPUB or comic.
#[derive(Debug)]
pub struct BookDoc {
    shape: Shape,
    source: Source,
    /// The rows of the Info tab.
    pub facts: Facts,
}

/// How a section fills the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Text in a reading column.
    Chapter,
    /// One picture, fitted to the room.
    Picture,
}

/// One section ready to show: the styles and the markup a sealed frame holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionPage {
    /// The section it is.
    pub section: SectionIndex,
    /// The chapter's sealed stylesheets; empty for a comic page.
    pub styles: String,
    /// How it fills the room.
    pub layout: Layout,
    /// The sealed markup: a chapter's content, or a comic page's picture.
    pub body: String,
}

impl BookDoc {
    /// How many chapters or pages the book has.
    pub fn sections(&self) -> SectionCount {
        match &self.shape {
            Shape::Epub { epub, .. } => epub.sections(),
            Shape::Comic(comic) => comic.sections(),
        }
    }

    /// The table of contents; none for a comic.
    pub fn contents(&self) -> &[TocEntry] {
        match &self.shape {
            Shape::Epub { contents, .. } => contents,
            Shape::Comic(_) => &[],
        }
    }

    /// The section `at`, unpacked and sealed. Blocking.
    pub fn section(&self, at: SectionIndex) -> Result<SectionPage, BookError> {
        match &self.shape {
            Shape::Epub { epub, .. } => {
                let chapter = epub.chapter(at)?;
                Ok(SectionPage {
                    section: at,
                    layout: Layout::Chapter,
                    styles: chapter.styles,
                    body: chapter.body,
                })
            }
            Shape::Comic(comic) => {
                let (page, bytes) = comic.page(at)?;
                let picture = format!(
                    "<img class=\"viewer-comic-page\" src=\"data:{};base64,{}\">",
                    page.mime,
                    ds_core::base64::encode(&bytes)
                );
                Ok(SectionPage {
                    section: at,
                    layout: Layout::Picture,
                    styles: String::new(),
                    body: picture,
                })
            }
        }
    }

    /// The file this is.
    pub fn source(&self) -> &Source {
        &self.source
    }
}

fn text_row(facts: Facts, label: FactLabel, text: Option<&String>) -> Facts {
    match text {
        Some(text) => facts.with(label, FactValue::text(text.clone())),
        None => facts,
    }
}

/// Open the book `src`. Blocking: reads the package or lists the pages.
pub(crate) fn open(
    src: &Source,
    sniffed: &Sniffed,
    _link: &OpenLink,
) -> Result<BookDoc, OpenError> {
    let base = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
        .with(FactLabel::Size, FactValue::size(src.stamp().len));
    let FormatDetail::Book(format) = sniffed.detail() else {
        return Err(OpenError::Unrecognised);
    };
    let (shape, facts) = match format {
        BookFormat::Cbz => {
            let comic = Comic::open(src.path())?;
            let facts = base.with(
                FactLabel::Pages,
                FactValue::text(comic.sections().get().to_string()),
            );
            (Shape::Comic(comic), facts)
        }
        BookFormat::Epub => {
            let epub = Epub::open(src.path())?;
            let meta = epub.meta();
            let facts = text_row(base, FactLabel::Title, meta.title.as_ref());
            let facts = text_row(facts, FactLabel::Author, meta.author.as_ref());
            let facts = text_row(facts, FactLabel::Publisher, meta.publisher.as_ref());
            let facts = text_row(facts, FactLabel::Language, meta.language.as_ref());
            let facts = facts.with(
                FactLabel::Chapters,
                FactValue::text(epub.sections().get().to_string()),
            );
            let contents = epub.contents();
            (Shape::Epub { epub, contents }, facts)
        }
    };
    Ok(BookDoc {
        shape,
        source: src.clone(),
        facts,
    })
}
