//! What a row of facts is about, and where each kind of row is listed by default.

use super::{FactGroup, Tier};
use ds_core::word::Word;

/// What a row of facts is about. Closed, so every pane words and orders the same facts the same
/// way and a label is never a free string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum FactLabel {
    /// What sort of file it is.
    Kind,
    /// How large the file is on disk.
    Size,
    /// When it last changed.
    Modified,
    /// When the file was made, where the file system keeps that.
    Created,
    /// The folder the file is in.
    Where,
    /// The address the file was downloaded from, as the browser recorded it.
    #[word(label = "Where from")]
    WhereFrom,
    /// Who may read and write the file.
    Permissions,
    /// Width and height of an image.
    Dimensions,
    /// How many pages a document has.
    Pages,
    /// How many slides a presentation has.
    Slides,
    /// How many sheets a workbook has.
    Sheets,
    /// How long a recording runs.
    Duration,
    /// How many frames an animation has.
    Frames,
    /// How the video, or the sound of an audio file, is compressed.
    Codec,
    /// How the sound of a video is compressed.
    AudioCodec,
    /// How many bits a second the streams of a recording take.
    Bitrate,
    /// How many pictures a second a video shows.
    Framerate,
    /// Samples a second of a recording's sound.
    SampleRate,
    /// How many channels a recording's sound has.
    Channels,
    /// What a recording holds: how many video, audio and subtitle tracks.
    Streams,
    /// How many chapters a recording is divided into.
    Chapters,
    /// The camera that took a photo.
    Camera,
    /// The lens a photo was taken with.
    Lens,
    /// The shutter and aperture of a photo, or all its settings where a pane has one row for them.
    Exposure,
    /// How far the exposure was moved from what the camera metered, in stops.
    #[word(label = "Exposure bias")]
    ExposureBias,
    /// The sensitivity a photo was taken at.
    #[word(label = "ISO")]
    Iso,
    /// The focal length of the lens for a photo, in millimetres.
    #[word(label = "Focal length")]
    FocalLength,
    /// Whether the flash fired.
    Flash,
    /// When a photo was taken.
    Taken,
    /// The program that made or last saved a file.
    Software,
    /// Who holds the rights to a picture.
    Copyright,
    /// How an image stores its colour: channels and bits per channel.
    Colour,
    /// How densely an image is meant to be printed or shown, in dots an inch.
    Resolution,
    /// Where a photo was taken: latitude and longitude.
    Coordinates,
    /// How high above sea level a photo was taken.
    Altitude,
    /// How many lines a text file has.
    Lines,
    /// How a text file's bytes are encoded.
    Encoding,
    /// How many rows a table has.
    Rows,
    /// How many columns a table has.
    Columns,
    /// The names of a structured value's top-level entries.
    Keys,
    /// How many entries an archive holds.
    Entries,
    /// The family a font belongs to.
    Family,
    /// A font's weight and slant by name: `Bold Italic`.
    Style,
    /// How many glyphs a font draws.
    Glyphs,
    /// A document's title.
    Title,
    /// A document's author, or the artist of a recording.
    Author,
    /// What a document is about, in the author's words.
    Subject,
    /// The words an author tagged a document with.
    Keywords,
    /// The program a document was written in.
    Creator,
    /// The program that made the file from the document.
    Producer,
    /// The version of the format a document is written in.
    Version,
    /// How big a document's pages are, named when a standard size.
    #[word(label = "Page size")]
    PageSize,
    /// What a document's protection allows: printing, copying, changing.
    Security,
    /// Whether a document carries its structure for screen readers.
    Tagged,
    /// The files embedded in a document.
    Attachments,
    /// The digital signatures on a document.
    Signatures,
    /// Who published a book.
    Publisher,
    /// The language a book is written in.
    Language,
    /// The album a recording belongs to.
    Album,
    /// Where a recording sits in its album, as the tag gives it.
    TrackNumber,
    /// A plugin the viewer lacks and the package that provides it.
    Needs,
}

impl FactLabel {
    /// The section a row with this label is listed in when its producer does not say.
    pub fn group(self) -> FactGroup {
        match self {
            FactLabel::Kind
            | FactLabel::Size
            | FactLabel::Modified
            | FactLabel::Created
            | FactLabel::Where
            | FactLabel::WhereFrom
            | FactLabel::Permissions
            | FactLabel::Needs => FactGroup::General,
            FactLabel::Dimensions
            | FactLabel::Frames
            | FactLabel::Colour
            | FactLabel::Resolution
            | FactLabel::Copyright => FactGroup::Picture,
            FactLabel::Camera
            | FactLabel::Lens
            | FactLabel::Exposure
            | FactLabel::ExposureBias
            | FactLabel::Iso
            | FactLabel::FocalLength
            | FactLabel::Flash
            | FactLabel::Taken
            | FactLabel::Software => FactGroup::Camera,
            FactLabel::Coordinates | FactLabel::Altitude => FactGroup::Location,
            FactLabel::Duration
            | FactLabel::Codec
            | FactLabel::Bitrate
            | FactLabel::Streams
            | FactLabel::Chapters => FactGroup::Media,
            FactLabel::AudioCodec
            | FactLabel::SampleRate
            | FactLabel::Channels
            | FactLabel::Album
            | FactLabel::TrackNumber => FactGroup::Audio,
            FactLabel::Framerate => FactGroup::Video,
            FactLabel::Pages
            | FactLabel::Slides
            | FactLabel::Sheets
            | FactLabel::Title
            | FactLabel::Author
            | FactLabel::Subject
            | FactLabel::Keywords
            | FactLabel::Creator
            | FactLabel::Producer
            | FactLabel::Version
            | FactLabel::PageSize
            | FactLabel::Security
            | FactLabel::Tagged
            | FactLabel::Attachments
            | FactLabel::Signatures => FactGroup::Document,
            FactLabel::Lines
            | FactLabel::Encoding
            | FactLabel::Rows
            | FactLabel::Columns
            | FactLabel::Keys => FactGroup::Text,
            FactLabel::Publisher | FactLabel::Language => FactGroup::Book,
            FactLabel::Family | FactLabel::Style | FactLabel::Glyphs => FactGroup::Font,
            FactLabel::Entries => FactGroup::Archive,
        }
    }

    /// Whether a row with this label is part of a file's summary line when its producer does not
    /// say.
    pub fn tier(self) -> Tier {
        match self {
            FactLabel::Kind
            | FactLabel::Size
            | FactLabel::Dimensions
            | FactLabel::Pages
            | FactLabel::Slides
            | FactLabel::Sheets
            | FactLabel::Duration
            | FactLabel::Frames
            | FactLabel::Colour
            | FactLabel::Lines
            | FactLabel::Rows
            | FactLabel::Columns
            | FactLabel::Entries
            | FactLabel::PageSize => Tier::Headline,
            FactLabel::Modified
            | FactLabel::Created
            | FactLabel::Where
            | FactLabel::WhereFrom
            | FactLabel::Permissions
            | FactLabel::Codec
            | FactLabel::AudioCodec
            | FactLabel::Bitrate
            | FactLabel::Framerate
            | FactLabel::SampleRate
            | FactLabel::Channels
            | FactLabel::Streams
            | FactLabel::Chapters
            | FactLabel::Camera
            | FactLabel::Lens
            | FactLabel::Exposure
            | FactLabel::ExposureBias
            | FactLabel::Iso
            | FactLabel::FocalLength
            | FactLabel::Flash
            | FactLabel::Taken
            | FactLabel::Software
            | FactLabel::Copyright
            | FactLabel::Resolution
            | FactLabel::Coordinates
            | FactLabel::Altitude
            | FactLabel::Encoding
            | FactLabel::Keys
            | FactLabel::Family
            | FactLabel::Style
            | FactLabel::Glyphs
            | FactLabel::Title
            | FactLabel::Author
            | FactLabel::Subject
            | FactLabel::Keywords
            | FactLabel::Creator
            | FactLabel::Producer
            | FactLabel::Version
            | FactLabel::Security
            | FactLabel::Tagged
            | FactLabel::Attachments
            | FactLabel::Signatures
            | FactLabel::Publisher
            | FactLabel::Language
            | FactLabel::Album
            | FactLabel::TrackNumber
            | FactLabel::Needs => Tier::Detail,
        }
    }
}
