//! Probing paths the way the launcher meets them: what each is, and how a zip is told from a document.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{BookFormat, ByteLen, FilePath, FormatDetail, FormatKind, OfficeFormat};
use anyview_peek::{PeekError, probe};
use std::io::Write;
use std::path::Path;
use support::{Home, path};

/// A zip of `entries` (name, bytes), stored, written as `name` in `dir`.
fn zip_at(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> FilePath {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (entry, bytes) in entries {
        writer.start_file(*entry, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    let file = dir.join(name);
    std::fs::write(&file, writer.finish().unwrap().into_inner()).unwrap();
    FilePath::new(file).unwrap()
}

#[test]
fn a_file_is_sniffed_from_its_head_and_stamped_as_it_is() {
    let png = path(Home::Image, "quadrants.png");
    let probed = probe(&FilePath::new(&png).unwrap()).unwrap();
    assert_eq!(probed.sniffed.kind(), FormatKind::Raster);
    let meta = std::fs::metadata(&png).unwrap();
    assert_eq!(probed.input.stamp().len, ByteLen(meta.len()));
    assert_ne!(probed.input.stamp().modified.0, 0);

    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "hello\nworld\n").unwrap();
    let probed = probe(&FilePath::new(&text).unwrap()).unwrap();
    assert_eq!(probed.sniffed.kind(), FormatKind::PlainText);
}

#[test]
fn a_folder_is_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let probed = probe(&FilePath::new(dir.path()).unwrap()).unwrap();
    assert_eq!(probed.sniffed.kind(), FormatKind::Folder);
}

#[test]
fn a_zip_is_told_from_a_document_by_what_is_inside() {
    let dir = tempfile::tempdir().unwrap();
    let docx = zip_at(
        dir.path(),
        "letter.docx",
        &[
            ("[Content_Types].xml", b"<Types/>"),
            ("word/document.xml", b"<w/>"),
        ],
    );
    let epub = zip_at(
        dir.path(),
        "novel.epub",
        &[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", b"<container/>"),
        ],
    );
    let plain = zip_at(
        dir.path(),
        "bundle.zip",
        &[("a.txt", b"a"), ("b.txt", b"b")],
    );
    // name, path, kind, detail
    let cases = [
        (
            "docx",
            docx,
            FormatKind::Office,
            FormatDetail::Office(OfficeFormat::Docx),
        ),
        (
            "epub",
            epub,
            FormatKind::Book,
            FormatDetail::Book(BookFormat::Epub),
        ),
        (
            "plain zip",
            plain,
            FormatKind::Archive,
            FormatDetail::Archive(anyview_core::ArchiveFormat::Zip),
        ),
    ];
    for (name, file, kind, detail) in cases {
        let probed = probe(&file).unwrap();
        assert_eq!(probed.sniffed.kind(), kind, "{name}");
        assert_eq!(probed.sniffed.detail(), &detail, "{name}");
    }
}

#[test]
fn a_zip_that_cannot_be_opened_is_a_plain_archive_for_the_peek_to_report_on() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.docx");
    std::fs::write(&broken, b"PK\x03\x04 then nothing a zip would hold").unwrap();
    let probed = probe(&FilePath::new(&broken).unwrap()).unwrap();
    assert_eq!(probed.sniffed.kind(), FormatKind::Archive);
}

#[test]
fn a_missing_path_is_missing_and_an_unreadable_one_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let absent = FilePath::new(dir.path().join("absent.png")).unwrap();
    assert!(
        matches!(probe(&absent), Err(PeekError::Missing { .. })),
        "{:?}",
        probe(&absent)
    );
    let locked = dir.path().join("locked.txt");
    std::fs::write(&locked, "secret").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let got = probe(&FilePath::new(&locked).unwrap());
    // A process that ignores permissions (root) reads it; anyone else is refused.
    if std::fs::File::open(&locked).is_err() {
        assert!(matches!(got, Err(PeekError::Unreadable { .. })), "{got:?}");
    }
}
