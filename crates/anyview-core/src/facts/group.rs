//! The sections a file's facts are listed in, and how much of them a summary line carries.

use ds_core::word::Word;

/// A section of the Info panel. Closed, so every pane names and orders the sections the same way.
/// The declaration order is the display order: the sections of what the file holds first, and
/// [`FactGroup::General`] (what any file has: its kind, size and dates) always last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Word)]
pub enum FactGroup {
    /// An image's pixels: its size, colour, resolution and frames.
    Picture,
    /// The camera, lens and settings a photo was taken with.
    Camera,
    /// Where a photo was taken. Only the viewer's own Info panel lists it, never a preview pane.
    Location,
    /// What any recording has: its length, streams and codecs.
    Media,
    /// The sound of a recording: tags, sample rate and channels.
    Audio,
    /// The picture of a recording: size, frame rate and colour.
    Video,
    /// A document: its title, author, dates, producer and pages.
    Document,
    /// A text file: encoding, lines, structure.
    Text,
    /// A book: its title, authors, publisher and chapters.
    Book,
    /// A font: its family, style and glyphs.
    Font,
    /// An archive: its entries and sizes.
    Archive,
    /// What every file has, kind, size, dates and where it is. Always the last section.
    General,
}

/// How much weight a row carries: a headline row is part of the one-line summary of a file, a
/// detail row is only listed in the full panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tier {
    /// In the summary line (`JPEG image · 4032 × 3024 · 3.2 MB`) and in the panel.
    Headline,
    /// Only in the panel.
    #[default]
    Detail,
}
