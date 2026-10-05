//! Office and iWork documents are zip packages: their title, author and size are in small parts
//! and a picture of the first page is often kept beside them. Both are read without unpacking
//! the document.

mod parts;
mod xml;

#[cfg(test)]
mod tests;

use crate::error::ArchiveError;
use crate::extract::{ExtractLimits, extract};
use anyview_core::{
    ArchiveFormat, ByteLen, FactLabel, FactValue, Facts, FilePath, OfficeFormat, PageCount,
};
use parts::{Metadata, parts_of};

/// The largest metadata part that is read.
const METADATA_BYTES: ByteLen = ByteLen(1024 * 1024);

/// The largest embedded picture that is read.
const THUMBNAIL_BYTES: ByteLen = ByteLen(4 * 1024 * 1024);

/// How a document's size is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfficeCount {
    /// Pages of a word-processing document.
    Pages(u32),
    /// Slides of a presentation.
    Slides(u32),
    /// Sheets of a workbook.
    Sheets(u32),
}

/// How an embedded picture is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailCodec {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
}

/// A picture of the document the document carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    /// The encoded picture.
    pub bytes: Vec<u8>,
    /// How it is encoded.
    pub codec: ThumbnailCodec,
}

/// What an office document says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OfficeLook {
    /// The document's title, when it has one.
    pub title: Option<String>,
    /// The author's name, when the document names one.
    pub author: Option<String>,
    /// Pages, slides or sheets, where the package says so.
    pub count: Option<OfficeCount>,
    /// The picture the document carries of itself.
    pub thumbnail: Option<Thumbnail>,
}

impl OfficeLook {
    /// The rows a pane lists: title, author and the count, each when the document has it.
    pub fn facts(&self) -> Facts {
        let facts = Facts::empty();
        let facts = match &self.title {
            Some(title) => facts.with(FactLabel::Title, FactValue::text(title.clone())),
            None => facts,
        };
        let facts = match &self.author {
            Some(author) => facts.with(FactLabel::Author, FactValue::text(author.clone())),
            None => facts,
        };
        match self.count {
            Some(OfficeCount::Pages(n)) => match PageCount::new(n) {
                Some(count) => facts.with(FactLabel::Pages, FactValue::pages(count)),
                None => facts,
            },
            Some(OfficeCount::Slides(n)) => {
                facts.with(FactLabel::Slides, FactValue::text(n.to_string()))
            }
            Some(OfficeCount::Sheets(n)) => {
                facts.with(FactLabel::Sheets, FactValue::text(n.to_string()))
            }
            None => facts,
        }
    }
}

fn part(path: &FilePath, name: &str, allowed: ByteLen) -> Option<Vec<u8>> {
    let limits = ExtractLimits {
        entry: allowed,
        scanned: allowed,
    };
    extract(path, ArchiveFormat::Zip, name, limits).ok()
}

/// What the office document at `path`, whose format sniffing named, says about itself. A format
/// that is not a zip package (the binary Word, Excel and PowerPoint) says nothing. Parts that are
/// missing or do not parse leave their fields empty: the facts the file does have still stand.
pub fn office_look(path: &FilePath, format: OfficeFormat) -> Result<OfficeLook, ArchiveError> {
    let Some(parts) = parts_of(format) else {
        return Ok(OfficeLook::default());
    };
    let mut look = OfficeLook::default();
    for source in parts.metadata {
        let Some(bytes) = part(path, source.name, METADATA_BYTES) else {
            continue;
        };
        let Metadata {
            title,
            author,
            count,
        } = source.read(&bytes);
        look.title = look.title.or(title);
        look.author = look.author.or(author);
        look.count = look.count.or(count);
    }
    look.thumbnail = parts.thumbnails.iter().find_map(|(name, codec)| {
        part(path, name, THUMBNAIL_BYTES).map(|bytes| Thumbnail {
            bytes,
            codec: *codec,
        })
    });
    Ok(look)
}
