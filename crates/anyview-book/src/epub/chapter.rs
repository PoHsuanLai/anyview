//! A chapter read from the zip and sealed.

use super::package::{Package, text_of};
use crate::error::BookError;
use crate::seal::{Assets, Chapter, seal};
use crate::zip_path::directory_of;
use crate::zip_read::read;
use anyview_archive::ArchiveError;
use anyview_core::work::Interrupt;
use anyview_core::{ByteLen, FileName, Input, SectionIndex, image_data_url};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// The largest chapter document that is read.
const CHAPTER_LIMIT: ByteLen = ByteLen(16 * 1024 * 1024);

/// The largest single image or stylesheet a chapter takes in.
const ASSET_LIMIT: ByteLen = ByteLen(8 * 1024 * 1024);

/// How much a chapter may take in altogether: each image becomes a `data:` URL in the page, and
/// each stylesheet is sealed into it.
const TOTAL_LIMIT: u64 = 24 * 1024 * 1024;

/// How many different files of the zip a chapter may ask for: each read finds its entry again in
/// the zip's index, and a name that is not there costs the same.
const READ_LIMIT: usize = 512;

pub(super) fn read_chapter(
    input: &Input,
    package: &Package,
    at: SectionIndex,
    interrupt: &dyn Interrupt,
) -> Result<Chapter, BookError> {
    let item = package
        .spine
        .get(at.0 as usize)
        .ok_or(BookError::NoSuchSection)?;
    let html = text_of(&read(input, &item.entry, CHAPTER_LIMIT, interrupt)?);
    let assets = ZipAssets {
        input,
        taken: Cell::new(0),
        asked: RefCell::default(),
        interrupt,
    };
    let chapter = seal(&html, directory_of(&item.entry), &assets);
    if interrupt.interrupted() {
        return Err(BookError::Archive(ArchiveError::Interrupted));
    }
    Ok(chapter)
}

/// The files of the zip, read as a chapter asks for them.
struct ZipAssets<'a> {
    input: &'a Input,
    taken: Cell<u64>,
    /// What each file asked for came to, so one asked for again is not read again.
    asked: RefCell<HashMap<(Kind, String), Option<String>>>,
    interrupt: &'a dyn Interrupt,
}

/// What a chapter takes a file as.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Image,
    Text,
}

impl ZipAssets<'_> {
    /// What `entry` comes to as `kind`, read once. Every byte read is charged to the chapter's
    /// total, whether or not the file turns out to be what was asked for.
    fn take(&self, kind: Kind, entry: &str) -> Option<String> {
        let key = (kind, entry.to_owned());
        if let Some(known) = self.asked.borrow().get(&key) {
            return known.clone();
        }
        if self.taken.get() >= TOTAL_LIMIT || self.asked.borrow().len() >= READ_LIMIT {
            return None;
        }
        let made = read(self.input, entry, ASSET_LIMIT, self.interrupt)
            .ok()
            .and_then(|bytes| {
                self.taken.set(self.taken.get() + bytes.len() as u64);
                match kind {
                    Kind::Image => FileName::new(entry.rsplit('/').next().unwrap_or(entry))
                        .ok()
                        .and_then(|name| image_data_url(&bytes, &name)),
                    Kind::Text => Some(text_of(&bytes)),
                }
            });
        self.asked.borrow_mut().insert(key, made.clone());
        made
    }
}

impl Assets for ZipAssets<'_> {
    fn data_url(&self, entry: &str) -> Option<String> {
        self.take(Kind::Image, entry)
    }

    fn text(&self, entry: &str) -> Option<String> {
        self.take(Kind::Text, entry)
    }
}
