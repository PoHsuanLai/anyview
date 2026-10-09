//! An EPUB laid out on a fixed reading page: each chapter, already sealed HTML with its images
//! inlined, is a PDF of its own, the chapters are bound in reading order, and the book's contents
//! become the outline.

use crate::io::OpenError;
use anyview_book::{BookError, Epub};
use anyview_core::{FactLabel, FactValue, Facts, FilePath, SectionIndex};
use anyview_fs::OnDisk;
use anyview_pdf::{Bookmark, bind as bind_parts};
use ds_blitz::{Margins, PageSize, PageSpec, Pt};
use std::collections::HashMap;

/// The book's page stylesheet.
const PAGE: &str = include_str!("book.css");

/// The reading page, in points: 680 of text between the margins, and the 2:3 of a book.
const MEASURE: f32 = 680.0;
const SIDE: f32 = 48.0;
const ACROSS: f32 = MEASURE + 2.0 * SIDE;
const DOWN: f32 = 1164.0;
const ABOVE: f32 = 56.0;

fn spec() -> PageSpec {
    PageSpec {
        size: PageSize::Custom {
            width: Pt(ACROSS),
            height: Pt(DOWN),
        },
        margins: Margins::symmetric(Pt(ABOVE), Pt(SIDE)),
    }
}

/// One chapter as the document it is laid out from: the book's own styles first, so the page's
/// rules (the reading face and measure) win where they meet.
pub(super) fn chapter_html(styles: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{styles}</style><style>{PAGE}</style></head><body>{body}</body></html>"
    )
}

fn text_row(facts: Facts, label: FactLabel, text: Option<&String>) -> Facts {
    match text {
        Some(text) => facts.with(label, FactValue::text(text.clone())),
        None => facts,
    }
}

/// The EPUB at `path` as the bytes of one PDF, and the rows of its Info tab (`base` with the
/// package's).
pub(super) fn bind(path: &FilePath, base: Facts) -> Result<(Vec<u8>, Facts), OpenError> {
    let epub = Epub::open(path.on_disk())?;
    let meta = epub.meta();
    let facts = text_row(base, FactLabel::Title, meta.title.as_ref());
    let facts = text_row(facts, FactLabel::Author, meta.author.as_ref());
    let facts = text_row(facts, FactLabel::Publisher, meta.publisher.as_ref());
    let facts = text_row(facts, FactLabel::Language, meta.language.as_ref());
    let facts = facts.with(
        FactLabel::Chapters,
        FactValue::text(epub.sections().get().to_string()),
    );

    // A chapter that cannot be read or laid out is left out; the rest of the book still reads.
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut part_of: Vec<Option<usize>> = Vec::new();
    for at in 0..epub.sections().get() {
        let laid = epub.chapter(SectionIndex(at)).ok().and_then(|chapter| {
            ds_blitz::pdf(&chapter_html(&chapter.styles, &chapter.body), spec()).ok()
        });
        part_of.push(laid.map(|bytes| {
            parts.push(bytes);
            parts.len() - 1
        }));
    }
    if parts.is_empty() {
        return Err(BookError::Empty.into());
    }
    let marks = marks(&epub, &part_of);
    let bytes = bind_parts(&parts, &marks).map_err(|_| BookError::Empty)?;
    Ok((bytes, facts))
}

/// The contents as bookmarks. Where several lines lead into one chapter (a book in a few long
/// files), each is looked for by its words; a chapter with one line goes to its first page.
fn marks(epub: &Epub, part_of: &[Option<usize>]) -> Vec<Bookmark> {
    let entries = epub.contents();
    let part = |section: Option<SectionIndex>| {
        section
            .and_then(|section| part_of.get(section.0 as usize).copied())
            .flatten()
    };
    let mut lines_into: HashMap<usize, usize> = HashMap::new();
    for entry in &entries {
        if let Some(part) = part(entry.section) {
            *lines_into.entry(part).or_default() += 1;
        }
    }
    entries
        .iter()
        .filter_map(|entry| {
            let part = part(entry.section)?;
            Some(Bookmark {
                title: entry.title.clone(),
                depth: usize::from(entry.depth),
                part,
                find: (lines_into.get(&part).copied().unwrap_or(0) > 1)
                    .then(|| entry.title.clone()),
            })
        })
        .collect()
}
