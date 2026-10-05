//! The media types of the kinds the viewer opens: the one map from kind to MIME that the desktop
//! entry's `MimeType` line is built from.

use super::family::Family;
use super::{BookFormat, Delimiter, FormatKind, MediaContainer, Mime, RasterFormat, TreeFormat};
use crate::peek::StageSupport;
use crate::profile::stage_support;
use ds_core::word::Word;

/// The media types a file of `kind` is sniffed as, in no order and with repeats allowed. Every
/// kind is answered: a kind with no type of its own (a folder, a file nothing describes) says none.
fn mimes_of(kind: FormatKind) -> Vec<&'static str> {
    fn family<F: Family>() -> Vec<&'static str> {
        F::ALL.iter().map(|format| format.mime()).collect()
    }
    match kind {
        FormatKind::Pdf => vec!["application/pdf"],
        FormatKind::Raster => family::<RasterFormat>(),
        FormatKind::Vector => vec!["image/svg+xml"],
        FormatKind::Video | FormatKind::Audio => MediaContainer::ALL
            .iter()
            .filter(|container| container.kind() == kind)
            .map(|container| container.mime())
            .collect(),
        FormatKind::Markdown => vec!["text/markdown"],
        FormatKind::Code => vec!["text/html", "text/css", "application/xml", "text/plain"],
        FormatKind::PlainText => vec!["text/plain"],
        FormatKind::Table => family::<Delimiter>(),
        FormatKind::Tree => family::<TreeFormat>(),
        FormatKind::Book => family::<BookFormat>(),
        FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => Vec::new(),
    }
}

/// The media types of every kind the viewer has a stage for (`StageSupport::Stage`), sorted and
/// without repeats: what a desktop entry claims the viewer opens. A kind the viewer only peeks
/// at is left out, since claiming it would make the viewer the answer to a file it cannot show.
pub fn opened_mimes() -> Vec<Mime> {
    let mut texts: Vec<&'static str> = FormatKind::ALL
        .iter()
        .copied()
        .filter(|kind| stage_support(*kind) == StageSupport::Stage)
        .flat_map(mimes_of)
        .collect();
    texts.sort_unstable();
    texts.dedup();
    texts.into_iter().map(Mime::known).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sniff::{FileHead, SniffStep, sniff};
    use crate::source::FileName;

    #[test]
    fn the_opened_types_are_those_of_the_kinds_with_a_stage() {
        let opened: Vec<String> = opened_mimes()
            .iter()
            .map(|m| m.as_str().to_owned())
            .collect();
        for want in [
            "application/pdf",
            "image/png",
            "image/svg+xml",
            "video/mp4",
            "audio/flac",
            "text/markdown",
            "text/plain",
            "text/csv",
            "application/json",
            "application/epub+zip",
            "application/vnd.comicbook+zip",
        ] {
            assert!(opened.iter().any(|m| m == want), "{want} is not opened");
        }
        for peeked in ["application/zip", "font/ttf", "inode/directory"] {
            assert!(
                !opened.iter().any(|m| m == peeked),
                "{peeked} is only peeked at"
            );
        }
        let mut sorted = opened.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(opened, sorted);
    }

    #[test]
    fn what_a_file_is_sniffed_as_is_in_the_map_of_its_kind() {
        const FILES: &[(&str, &[u8])] = &[
            ("a.pdf", b"%PDF-1.7\n"),
            ("a.svg", b"<svg/>"),
            ("a.md", b"# t\n"),
            ("a.html", b"<p>x</p>"),
            ("a.css", b"a{}"),
            ("a.xml", b"<a/>"),
            ("a.rs", b"fn main() {}"),
            ("a.txt", b"text"),
            ("a.csv", b"a,b\n"),
            ("a.json", b"{}"),
            ("a.jsonl", b"{}\n"),
        ];
        for (file, head) in FILES {
            let name = FileName::new(file).unwrap();
            let SniffStep::Done(sniffed) = sniff(&FileHead::new(head), &name) else {
                panic!("{file} needs a look inside");
            };
            assert!(
                mimes_of(sniffed.kind()).contains(&sniffed.mime().as_str()),
                "{file}: {:?} as {}",
                sniffed.kind(),
                sniffed.mime().as_str()
            );
        }
    }

    #[test]
    fn every_opened_type_names_a_kind_with_a_stage() {
        use crate::kind::kind_of_mime;
        for mime in opened_mimes() {
            let kind = kind_of_mime(&mime);
            assert_eq!(
                stage_support(kind),
                StageSupport::Stage,
                "{}",
                mime.as_str()
            );
        }
    }
}
