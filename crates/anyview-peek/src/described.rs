//! The peek of a kind whose back end does not exist yet: what the file is, from its sniffed type.
//!
//! Files of a type nothing opens are each mapped from day one (ARCHITECTURE section 3, rule 3).
//! Until their back ends land they show what sniffing established, plus the size and date every pane lists, and nothing is
//! pretended: no duration, no cover. A back end replaces its marker's `Peek` impl
//! with a real one and the registry's arm names the new type.

use anyview_core::{
    FactLabel, FactValue, Facts, FormatKind, Input, Peek, PeekBudget, Sniffed, kind_name_of,
};
use std::convert::Infallible;
use std::marker::PhantomData;

/// What a facts-only peek holds: the words for the file's type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    /// What the file is, as a person names it: `MP4 video`, `Word document`, `BIN file`. Never a
    /// media type.
    pub kind: String,
}

impl Described {
    /// What `sniffed` says the file `src` is.
    pub fn of(src: &Input, sniffed: &Sniffed) -> Self {
        Described {
            kind: kind_name_of(sniffed, src.name()),
        }
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

    fn peek(src: &Input, sniffed: &Sniffed, _: &PeekBudget) -> Result<Described, Infallible> {
        Ok(Described::of(src, sniffed))
    }

    fn facts(peeked: &Described) -> Facts {
        Facts::empty().with(FactLabel::Kind, FactValue::text(peeked.kind.clone()))
    }
}

// The kinds that have no back end yet, one unit type each, for [`FactsPeek`] to be generic over.

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
    fn a_file_is_described_in_a_persons_words_never_by_its_media_type() {
        // name, file name, head, words
        const CASES: &[(&str, &str, &[u8], &str)] = &[
            (
                "mp4",
                "clip.mp4",
                b"\0\0\0\x18ftypmp42\0\0\0\0mp42isom",
                "MP4 video",
            ),
            ("flac", "song.flac", b"fLaC\0\0\0\x22", "FLAC audio"),
            (
                "ttf",
                "face.ttf",
                b"\0\x01\0\0\0\x0f\0\x80\0\x03\0\x30",
                "TrueType font",
            ),
            (
                "no known kind",
                "random.bin",
                b"\x01\x02\0\0\xfe\xfd",
                "BIN file",
            ),
            ("no extension", "blob", b"\x01\x02\0\0\xfe\xfd", "Document"),
        ];
        for (name, file, head, want) in CASES {
            let stamp = anyview_core::FileStamp {
                len: anyview_core::ByteLen(head.len() as u64),
                modified: anyview_core::ModTime(0),
            };
            let src = Input::new(
                FileName::new(file).unwrap(),
                stamp,
                std::sync::Arc::new(head.to_vec()),
            );
            assert_eq!(
                Described::of(&src, &sniffed(file, head)).kind,
                *want,
                "{name}"
            );
        }
    }
}
