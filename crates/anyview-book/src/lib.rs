//! Books for the viewer: an EPUB's package (metadata, reading order, contents) and its chapters as
//! sealed HTML, a comic zip's pages in natural order, and the covers of both.
//!
//! Blocking and effect-free except the reads of the one file a book is: no runtime, no spawning,
//! no clock. The zip is read through `anyview-archive`, the one crate that names the codecs.
//!
//! Every public item is reached from this root, once.

#![warn(missing_docs)]

mod comic;
mod cover;
mod epub;
mod error;
mod natural;
mod seal;
mod zip_path;
mod zip_read;

pub use comic::{Comic, ComicPage};
pub use cover::{Cover, comic_cover, epub_cover};
pub use epub::{Epub, EpubMeta, TocEntry};
pub use error::BookError;
pub use natural::natural_order;
pub use seal::Chapter;
