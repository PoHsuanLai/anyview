//! An EPUB: its package read from the zip (metadata, reading order, cover), its table of contents
//! and its chapters as sealed HTML.

mod chapter;
mod contents;
mod package;

pub use contents::TocEntry;

use crate::error::BookError;
use crate::seal::Chapter;
use crate::zip_read::read;
use anyview_core::work::Interrupt;
use anyview_core::{ByteLen, Deadline, Input, SectionCount, SectionIndex};
use package::Package;

/// The largest package part (container, package document, contents) that is read.
const PART_LIMIT: ByteLen = ByteLen(8 * 1024 * 1024);

/// What the package says of the book.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpubMeta {
    /// Its title.
    pub title: Option<String>,
    /// Its authors, joined by commas.
    pub author: Option<String>,
    /// Who published it.
    pub publisher: Option<String>,
    /// The language code it is written in.
    pub language: Option<String>,
}

/// An opened EPUB: where it is and what its package says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epub {
    input: Input,
    package: Package,
    count: SectionCount,
}

impl Epub {
    /// The EPUB `input` (a path, or any bytes a host injects): its container and package document. A book with no chapter is
    /// [`BookError::Empty`].
    pub fn open(input: impl Into<Input>) -> Result<Epub, BookError> {
        let input = input.into();
        let container = read(
            &input,
            "META-INF/container.xml",
            PART_LIMIT,
            &Deadline::NEVER,
        )?;
        let package_path = package::rootfile(&container)?;
        let document = read(&input, &package_path, PART_LIMIT, &Deadline::NEVER)?;
        let package = package::parse(&package_path, &document)?;
        let count = u32::try_from(package.spine.len())
            .ok()
            .and_then(SectionCount::new)
            .ok_or(BookError::Empty)?;
        Ok(Epub {
            input,
            package,
            count,
        })
    }

    /// What the package says of the book.
    pub fn meta(&self) -> &EpubMeta {
        &self.package.meta
    }

    /// How many chapters the reading order has.
    pub fn sections(&self) -> SectionCount {
        self.count
    }

    /// The entry of the cover image, when the package names one.
    pub fn cover_entry(&self) -> Option<&str> {
        self.package.cover.as_deref()
    }

    /// The table of contents, nested by depth; the reading order named by chapter when the book
    /// has none. Blocking.
    pub fn contents(&self) -> Vec<TocEntry> {
        contents::read(&self.input, &self.package)
    }

    /// The chapter `at` as sealed HTML. Blocking.
    pub fn chapter(&self, at: SectionIndex) -> Result<Chapter, BookError> {
        self.chapter_until(at, &Deadline::NEVER)
    }

    /// [`Epub::chapter`], ended with an interrupted archive error as soon as `interrupt` says so:
    /// it is asked before each part of the zip is unpacked and once the chapter is sealed, so a
    /// chapter that lost an image to it is never returned as whole. Blocking.
    pub fn chapter_until(
        &self,
        at: SectionIndex,
        interrupt: &dyn Interrupt,
    ) -> Result<Chapter, BookError> {
        chapter::read_chapter(&self.input, &self.package, at, interrupt)
    }

    /// The bytes of the entry `entry`, for a cover. Blocking.
    pub(crate) fn entry(&self, entry: &str, allowed: ByteLen) -> Result<Vec<u8>, BookError> {
        read(&self.input, entry, allowed, &Deadline::NEVER)
    }
}
