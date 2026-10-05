//! The peek of a kind whose back end does not exist yet: what the file is, from its sniffed type.
//!
//! Books and files of a type nothing opens are
//! each mapped from day one (ARCHITECTURE section 3, rule 3). Until their back ends land they
//! show what sniffing established, plus the size and date every pane lists, and nothing is
//! pretended: no duration, no cover. A back end replaces its marker's `Peek` impl
//! with a real one and the registry's arm names the new type.

use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, Sniffed, Source,
};
use ds::prelude::Word;
use std::convert::Infallible;
use std::marker::PhantomData;

/// What a facts-only peek holds: the words for the file's type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    /// What the file is: `Video (MP4)`, `Office (DOCX)`, or its media type when the kind has no
    /// formats to name (`application/pdf`).
    pub kind: String,
}

impl Described {
    /// What `sniffed` says the file is.
    pub fn of(sniffed: &Sniffed) -> Self {
        let kind = match format_label(sniffed.detail()) {
            Some(format) => format!("{} ({})", sniffed.kind().label(), format.to_uppercase()),
            None => sniffed.mime().as_str().to_owned(),
        };
        Described { kind }
    }
}

/// The name of the format inside a kind, for the kinds whose detail is a format.
fn format_label(detail: &FormatDetail) -> Option<&'static str> {
    match detail {
        FormatDetail::Media(format) => Some(format.label()),
        FormatDetail::Font(format) => Some(format.label()),
        FormatDetail::Archive(format) => Some(format.label()),
        FormatDetail::Book(format) => Some(format.label()),
        FormatDetail::Office(format) => Some(format.label()),
        FormatDetail::None
        | FormatDetail::Raster(_)
        | FormatDetail::Code(_)
        | FormatDetail::Table(_)
        | FormatDetail::Tree(_)
        | FormatDetail::Text(_) => None,
    }
}

/// A kind that has only the facts peek.
pub trait Describes: 'static {
    /// The kind.
    const KIND: FormatKind;
}

/// The facts-only peek of the kind `K` names.
#[derive(Debug, Clone, Copy)]
pub struct FactsPeek<K>(PhantomData<K>);

impl<K: Describes> Peek for FactsPeek<K> {
    const KIND: FormatKind = K::KIND;
    type Peeked = Described;
    type Error = Infallible;

    fn peek(_: &Source, sniffed: &Sniffed, _: &PeekBudget) -> Result<Described, Infallible> {
        Ok(Described::of(sniffed))
    }

    fn facts(peeked: &Described) -> Facts {
        Facts::empty().with(FactLabel::Kind, FactValue::text(peeked.kind.clone()))
    }
}

// The kinds that have no back end yet, one unit type each, for [`FactsPeek`] to be generic over.

/// The kind `Book`.
#[derive(Debug, Clone, Copy)]
pub struct BookKind;

impl Describes for BookKind {
    const KIND: FormatKind = FormatKind::Book;
}

/// The facts-only peek of the kind `Book`.
pub type BookPeek = FactsPeek<BookKind>;

/// The kind `Other`.
#[derive(Debug, Clone, Copy)]
pub struct OtherKind;

impl Describes for OtherKind {
    const KIND: FormatKind = FormatKind::Other;
}

/// The facts-only peek of the kind `Other`.
pub type OtherPeek = FactsPeek<OtherKind>;

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FileHead, FileName, SniffStep, sniff};

    fn sniffed(name: &str, head: &[u8]) -> Sniffed {
        match sniff(&FileHead::new(head), &FileName::new(name).unwrap()) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(_) => panic!("{name} is answered from its head"),
        }
    }

    #[test]
    fn a_file_is_described_by_its_kind_and_format() {
        // name, file name, head, words
        const CASES: &[(&str, &str, &[u8], &str)] = &[
            (
                "mp4",
                "clip.mp4",
                b"\0\0\0\x18ftypmp42\0\0\0\0mp42isom",
                "Video (MP4)",
            ),
            ("flac", "song.flac", b"fLaC\0\0\0\x22", "Audio (FLAC)"),
            (
                "ttf",
                "face.ttf",
                b"\0\x01\0\0\0\x0f\0\x80\0\x03\0\x30",
                "Font (TTF)",
            ),
        ];
        for (name, file, head, want) in CASES {
            assert_eq!(Described::of(&sniffed(file, head)).kind, *want, "{name}");
        }
    }
}
