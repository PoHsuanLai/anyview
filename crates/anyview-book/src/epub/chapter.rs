//! A chapter read from the zip and sealed.

use super::package::{Package, text_of};
use crate::error::BookError;
use crate::seal::{Assets, Chapter, seal};
use crate::zip_path::directory_of;
use crate::zip_read::read;
use anyview_core::{
    ByteLen, FileHead, FileName, FilePath, FormatKind, SectionIndex, SniffStep, sniff,
};
use std::cell::Cell;

/// The largest chapter document that is read.
const CHAPTER_LIMIT: ByteLen = ByteLen(16 * 1024 * 1024);

/// The largest single image or stylesheet a chapter takes in.
const ASSET_LIMIT: ByteLen = ByteLen(8 * 1024 * 1024);

/// How much a chapter may take in altogether: each image becomes a `data:` URL in the page, and
/// each stylesheet is sealed into it.
const TOTAL_LIMIT: u64 = 24 * 1024 * 1024;

pub(super) fn read_chapter(
    path: &FilePath,
    package: &Package,
    at: SectionIndex,
) -> Result<Chapter, BookError> {
    let item = package
        .spine
        .get(at.0 as usize)
        .ok_or(BookError::NoSuchSection)?;
    let html = text_of(&read(path, &item.entry, CHAPTER_LIMIT)?);
    let assets = ZipAssets {
        path,
        taken: Cell::new(0),
    };
    Ok(seal(&html, directory_of(&item.entry), &assets))
}

/// The files of the zip, read as a chapter asks for them.
struct ZipAssets<'a> {
    path: &'a FilePath,
    taken: Cell<u64>,
}

impl Assets for ZipAssets<'_> {
    fn data_url(&self, entry: &str) -> Option<String> {
        if self.taken.get() >= TOTAL_LIMIT {
            return None;
        }
        let bytes = read(self.path, entry, ASSET_LIMIT).ok()?;
        let mime = image_mime(entry, &bytes)?;
        self.taken.set(self.taken.get() + bytes.len() as u64);
        Some(format!(
            "data:{mime};base64,{}",
            ds_core::base64::encode(&bytes)
        ))
    }

    fn text(&self, entry: &str) -> Option<String> {
        if self.taken.get() >= TOTAL_LIMIT {
            return None;
        }
        let bytes = read(self.path, entry, ASSET_LIMIT).ok()?;
        self.taken.set(self.taken.get() + bytes.len() as u64);
        Some(text_of(&bytes))
    }
}

/// The media type of the picture `bytes` holds, or `None` when the entry is not a picture.
fn image_mime(entry: &str, bytes: &[u8]) -> Option<String> {
    let name = FileName::new(entry.rsplit('/').next().unwrap_or(entry)).ok()?;
    let head = FileHead::new(&bytes[..bytes.len().min(4096)]);
    match sniff(&head, &name) {
        SniffStep::Done(sniffed)
            if matches!(sniffed.kind(), FormatKind::Raster | FormatKind::Vector) =>
        {
            Some(sniffed.mime().as_str().to_owned())
        }
        SniffStep::Done(_) | SniffStep::LookInside(_) => None,
    }
}
