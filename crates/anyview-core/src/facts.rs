//! Facts: the label and value rows a pane lists about a file (dimensions, pages, duration…).

use crate::source::ByteLen;
use crate::units::{Bitrate, MediaLength, PageCount, PixelSize};
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
    /// Width and height of an image.
    Dimensions,
    /// How many pages a document has.
    Pages,
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
    /// The camera that took a photo.
    Camera,
    /// The lens a photo was taken with.
    Lens,
    /// The shutter, aperture and sensitivity of a photo.
    Exposure,
    /// When a photo was taken.
    Taken,
    /// How an image stores its colour: channels and bits per channel.
    Colour,
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
    /// A document's title.
    Title,
    /// A document's author, or the artist of a recording.
    Author,
    /// The album a recording belongs to.
    Album,
}

/// The text shown for a fact, already formatted for a person. A producer builds one from its typed
/// value with the constructors here, so a pane never formats a number itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FactValue(String);

impl FactValue {
    /// A value that is already text (a title, a codec name, a date the caller formatted).
    pub fn text(text: impl Into<String>) -> Self {
        FactValue(text.into())
    }

    /// `1920 × 1080`.
    pub fn dimensions(size: PixelSize) -> Self {
        FactValue(format!("{} × {}", size.width.0, size.height.0))
    }

    /// `1 page` or `12 pages`.
    pub fn pages(count: PageCount) -> Self {
        let n = count.get();
        FactValue(format!("{n} {}", if n == 1 { "page" } else { "pages" }))
    }

    /// `3:07`, or `1:02:03` from one hour; seconds are rounded down.
    pub fn duration(length: MediaLength) -> Self {
        let secs = length.0.0 / 1_000_000;
        let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
        FactValue(if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        })
    }

    /// `192 kbit/s`.
    pub fn bitrate(rate: Bitrate) -> Self {
        FactValue(format!("{} kbit/s", rate.kbps()))
    }

    /// A size in decimal units with one digit after the point, rounded down: `412 B`, `1.5 KB`,
    /// `12.3 MB`.
    pub fn size(len: ByteLen) -> Self {
        const UNITS: &[(&str, u128)] = &[
            ("TB", 1_000_000_000_000),
            ("GB", 1_000_000_000),
            ("MB", 1_000_000),
            ("KB", 1_000),
        ];
        let bytes = u128::from(len.0);
        let text = UNITS
            .iter()
            .find(|(_, unit)| bytes >= *unit)
            .map(|(name, unit)| {
                let tenths = bytes * 10 / unit;
                format!("{}.{} {name}", tenths / 10, tenths % 10)
            })
            .unwrap_or_else(|| format!("{bytes} B"));
        FactValue(text)
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One row: a label and its value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fact {
    /// What the row is about.
    pub label: FactLabel,
    /// What is true of it.
    pub value: FactValue,
}

/// The rows a pane lists, in the order the producer gave them. Empty is a file with nothing to say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts(Vec<Fact>);

impl Facts {
    /// No rows.
    pub fn empty() -> Self {
        Facts(Vec::new())
    }

    /// These rows plus one more at the end.
    pub fn with(mut self, label: FactLabel, value: FactValue) -> Self {
        self.0.push(Fact { label, value });
        self
    }

    /// The rows in order.
    pub fn rows(&self) -> &[Fact] {
        &self.0
    }

    /// The value of the first row labelled `label`.
    pub fn value(&self, label: FactLabel) -> Option<&FactValue> {
        self.0
            .iter()
            .find(|fact| fact.label == label)
            .map(|fact| &fact.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{MediaTime, PixelLen};

    #[test]
    fn sizes_are_decimal_with_one_digit() {
        const CASES: &[(&str, u64, &str)] = &[
            ("zero", 0, "0 B"),
            ("bytes", 999, "999 B"),
            ("one kilobyte", 1_000, "1.0 KB"),
            ("kilobytes round down", 1_599, "1.5 KB"),
            ("just under a megabyte", 999_999, "999.9 KB"),
            ("one megabyte", 1_000_000, "1.0 MB"),
            ("megabytes", 12_345_678, "12.3 MB"),
            ("gigabytes", 4_000_000_000, "4.0 GB"),
            ("terabytes", 2_500_000_000_000, "2.5 TB"),
            ("the largest", u64::MAX, "18446744.0 TB"),
        ];
        for (name, bytes, want) in CASES {
            assert_eq!(FactValue::size(ByteLen(*bytes)).as_str(), *want, "{name}");
        }
    }

    #[test]
    fn durations_read_as_clock_time() {
        const CASES: &[(&str, u64, &str)] = &[
            ("zero", 0, "0:00"),
            ("under a second", 999_999, "0:00"),
            ("seconds", 7_000_000, "0:07"),
            ("minutes", 187_000_000, "3:07"),
            ("an hour", 3_600_000_000, "1:00:00"),
            ("hours", 3_723_000_000, "1:02:03"),
        ];
        for (name, micros, want) in CASES {
            let length = MediaLength(MediaTime(*micros));
            assert_eq!(FactValue::duration(length).as_str(), *want, "{name}");
        }
    }

    #[test]
    fn dimensions_and_pages_read_naturally() {
        let size = PixelSize {
            width: PixelLen(1920),
            height: PixelLen(1080),
        };
        assert_eq!(FactValue::dimensions(size).as_str(), "1920 × 1080");
        assert_eq!(
            FactValue::pages(PageCount::new(1).unwrap()).as_str(),
            "1 page"
        );
        assert_eq!(
            FactValue::pages(PageCount::new(12).unwrap()).as_str(),
            "12 pages"
        );
    }

    #[test]
    fn rows_keep_their_order_and_are_found_by_label() {
        let facts = Facts::empty()
            .with(FactLabel::Kind, FactValue::text("PNG image"))
            .with(FactLabel::Size, FactValue::size(ByteLen(2_000)))
            .with(FactLabel::Kind, FactValue::text("shadowed"));
        let labels: Vec<_> = facts.rows().iter().map(|f| f.label).collect();
        assert_eq!(labels, [FactLabel::Kind, FactLabel::Size, FactLabel::Kind]);
        assert_eq!(
            facts.value(FactLabel::Kind),
            Some(&FactValue::text("PNG image"))
        );
        assert_eq!(
            facts.value(FactLabel::Size).map(FactValue::as_str),
            Some("2.0 KB")
        );
        assert_eq!(facts.value(FactLabel::Pages), None);
        assert!(Facts::empty().rows().is_empty());
        assert_eq!(Facts::default(), Facts::empty());
    }
}
