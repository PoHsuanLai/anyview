//! Books and comics: documents read page by page that are zip files inside.

use super::family::Family;
use ds_core::word::Word;

/// A book format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum BookFormat {
    /// An EPUB book.
    Epub,
    /// A comic: a zip of images.
    Cbz,
}

impl Family for BookFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            BookFormat::Epub => &["epub"],
            BookFormat::Cbz => &["cbz"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            BookFormat::Epub => "application/epub+zip",
            BookFormat::Cbz => "application/vnd.comicbook+zip",
        }
    }
}
