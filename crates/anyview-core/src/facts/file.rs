//! The General section: what every file has, whatever it holds. The file system is read at the
//! edge (`anyview-store`); this turns what it said into rows.

use super::{FactTime, FactValue, Facts, kind_name};
use crate::facts::FactLabel;
use crate::kind::FormatKind;
use crate::sniff::Sniffed;
use crate::source::{ByteLen, ModTime};

/// What the file system says about one file besides its bytes. A field the file system did not
/// give is absent, and its row is left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDetails {
    /// The size in bytes.
    pub len: Option<ByteLen>,
    /// When the file was made, where the file system records it.
    pub created: Option<ModTime>,
    /// When it last changed.
    pub modified: Option<ModTime>,
    /// The folder it is in, as a path.
    pub folder: Option<String>,
    /// The address it was downloaded from, as the browser stored it in its extended attribute.
    pub origin: Option<String>,
    /// The permission bits (`0o644`).
    pub mode: Option<u32>,
}

impl Facts {
    /// The General section of the file `sniffed` is, as the file system describes it: kind, size
    /// with its exact bytes, created, modified, where, where from and permissions, in that order.
    pub fn general(sniffed: &Sniffed, details: &FileDetails) -> Facts {
        let date = |time: ModTime| FactValue::date(FactTime::from_mod_time(time));
        let mut facts = Facts::empty().with(FactLabel::Kind, FactValue::text(kind_name(sniffed)));
        // A folder's size is what its contents add up to, which only its own peek knows.
        if let Some(len) = details.len
            && sniffed.kind() != FormatKind::Folder
        {
            facts = facts.with(FactLabel::Size, FactValue::size_exact(len));
        }
        if let Some(created) = details.created {
            facts = facts.with(FactLabel::Created, date(created));
        }
        if let Some(modified) = details.modified {
            facts = facts.with(FactLabel::Modified, date(modified));
        }
        if let Some(folder) = &details.folder {
            facts = facts.with(FactLabel::Where, FactValue::text(folder.clone()));
        }
        if let Some(from) = details.origin.as_deref().and_then(origin_text) {
            facts = facts.with(FactLabel::WhereFrom, FactValue::text(from));
        }
        match details.mode {
            Some(mode) => facts.with(FactLabel::Permissions, FactValue::text(permissions(mode))),
            None => facts,
        }
    }
}

/// `example.com/files/report.pdf` for `https://user:pw@example.com/files/report.pdf?token=x#top`:
/// the address host first, with the scheme, the login, the query and the fragment left out (a
/// download link often carries a token). `None` for text that is not an address.
fn origin_text(url: &str) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let known = !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = authority.rsplit('@').next().unwrap_or("");
    (known && !host.is_empty()).then(|| {
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            host.to_owned()
        } else {
            format!("{host}/{path}")
        }
    })
}

/// `Read and write (rw-r--r--)`: what the owner may do, then the bits.
fn permissions(mode: u32) -> String {
    let owner = match (mode & 0o400 != 0, mode & 0o200 != 0) {
        (true, true) => "Read and write",
        (true, false) => "Read only",
        (false, true) => "Write only",
        (false, false) => "No access",
    };
    let bits: String = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ]
    .iter()
    .map(|(bit, letter)| if mode & bit != 0 { *letter } else { '-' })
    .collect();
    format!("{owner} ({bits})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::FactGroup;
    use crate::sniff::{FileHead, SniffStep, sniff};
    use crate::source::FileName;

    fn sniffed(name: &str, head: &[u8]) -> Sniffed {
        match sniff(&FileHead::new(head), &FileName::new(name).unwrap()) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(_) => panic!("{name} is a zip"),
        }
    }

    #[test]
    fn kinds_are_named_as_finder_names_them() {
        const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
        const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\0\x10JFIF\0\x01\x01\0\0\x01\0\x01\0\0";
        const PDF: &[u8] = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";
        const FLAC: &[u8] = b"fLaC\0\0\0\x22";
        const MP4: &[u8] = b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2mp41";
        // name, head, words
        const CASES: &[(&str, &[u8], &str)] = &[
            ("a.png", PNG, "PNG image"),
            ("a.jpg", JPEG, "JPEG image"),
            ("a.pdf", PDF, "PDF document"),
            ("a.flac", FLAC, "FLAC audio"),
            ("a.mp4", MP4, "MP4 video"),
            ("main.rs", b"fn main() {}\n", "Rust source"),
            ("app.js", b"let x = 1;\n", "JavaScript source"),
            ("notes.txt", b"hello\n", "Plain text"),
            ("README.md", b"# Title\n", "Markdown document"),
            ("data.csv", b"a,b\n1,2\n", "CSV table"),
            ("data.json", b"{\"a\":1}", "JSON document"),
        ];
        for (name, head, want) in CASES {
            assert_eq!(kind_name(&sniffed(name, head)), *want, "{name}");
        }
    }

    #[test]
    fn where_from_is_host_first_with_no_query_login_or_scheme() {
        // name, raw, words
        const CASES: &[(&str, &str, Option<&str>)] = &[
            (
                "a token in the query",
                "https://example.com/files/report.pdf?token=abc#top",
                Some("example.com/files/report.pdf"),
            ),
            (
                "a login",
                "ftp://user:secret@host.example:2121/pub/a.zip",
                Some("host.example:2121/pub/a.zip"),
            ),
            ("a bare host", "https://example.com/", Some("example.com")),
            (
                "a bare host with a query",
                "https://example.com?x=1",
                Some("example.com"),
            ),
            ("not an address", "report final", None),
            ("no host", "file:///tmp/a.pdf", None),
            ("empty", "", None),
        ];
        for (name, raw, want) in CASES {
            assert_eq!(origin_text(raw).as_deref(), *want, "{name}");
        }
    }

    #[test]
    fn permissions_say_what_the_owner_may_do() {
        assert_eq!(permissions(0o644), "Read and write (rw-r--r--)");
        assert_eq!(permissions(0o400), "Read only (r--------)");
        assert_eq!(permissions(0o755), "Read and write (rwxr-xr-x)");
        assert_eq!(permissions(0o200), "Write only (-w-------)");
        assert_eq!(permissions(0o000), "No access (---------)");
    }

    #[test]
    fn the_general_section_lists_what_the_file_system_gave_in_order() {
        let details = FileDetails {
            len: Some(ByteLen(3_214_880)),
            created: Some(ModTime(1_790_951_400 * 1_000_000_000)),
            modified: Some(ModTime(1_790_951_460 * 1_000_000_000)),
            folder: Some("/home/me/Pictures".to_owned()),
            origin: Some("https://example.com/p.jpg?sig=1".to_owned()),
            mode: Some(0o644),
        };
        let facts = Facts::general(&sniffed("p.jpg", b"\xFF\xD8\xFF\xE0\0\x10JFIF\0"), &details);
        let rows: Vec<(FactLabel, &str)> = facts
            .rows()
            .iter()
            .map(|row| (row.label, row.value.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (FactLabel::Kind, "JPEG image"),
                (FactLabel::Size, "3.2 MB (3,214,880 bytes)"),
                (FactLabel::Created, "2 Oct 2026 at 14:30 UTC"),
                (FactLabel::Modified, "2 Oct 2026 at 14:31 UTC"),
                (FactLabel::Where, "/home/me/Pictures"),
                (FactLabel::WhereFrom, "example.com/p.jpg"),
                (FactLabel::Permissions, "Read and write (rw-r--r--)"),
            ]
        );
        assert!(
            facts
                .rows()
                .iter()
                .all(|row| row.group == FactGroup::General)
        );
    }

    #[test]
    fn a_folder_has_no_size_of_its_own() {
        let details = FileDetails {
            len: Some(ByteLen(4096)),
            ..FileDetails::default()
        };
        let facts = Facts::general(&crate::sniff::sniff_folder(), &details);
        let labels: Vec<_> = facts.rows().iter().map(|row| row.label).collect();
        assert_eq!(labels, [FactLabel::Kind]);
        assert_eq!(
            facts.value(FactLabel::Kind).map(FactValue::as_str),
            Some("Folder")
        );
    }

    #[test]
    fn what_the_file_system_did_not_say_is_left_out() {
        let details = FileDetails {
            len: Some(ByteLen(5)),
            ..FileDetails::default()
        };
        let facts = Facts::general(&sniffed("a.txt", b"hello"), &details);
        let labels: Vec<_> = facts.rows().iter().map(|row| row.label).collect();
        assert_eq!(labels, [FactLabel::Kind, FactLabel::Size]);
    }
}
