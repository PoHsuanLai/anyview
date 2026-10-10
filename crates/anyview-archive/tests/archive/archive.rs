//! Listing and extracting through the public API, on archives built in a scratch folder.

use crate::support;

use anyview_archive::{
    ArchiveError, ArchivePeek, EntryCount, EntryKind, EntryLimit, ExtractLimits, Holds, extract,
    list, zip_entries,
};
use anyview_core::{
    ArchiveFormat, ByteLen, FactLabel, FileHead, FileName, FileStamp, FormatKind, ModTime, Peek,
    PeekBudget, PixelArea, SniffStep, Source, ZipEntries, sniff,
};
use anyview_fs::OnDisk;
use std::time::Duration;
use support::{ENTRIES, bzip2, gzip, sevenz_of, tar_of, write, write_path, xz, zip_of, zstd};

const BIG: ByteLen = ByteLen(1 << 20);
const ALL: EntryLimit = EntryLimit(1000);
const LIMITS: ExtractLimits = ExtractLimits {
    entry: BIG,
    scanned: BIG,
};

/// Every container holding [`ENTRIES`] (and a link for the tars), with the format it sniffs as.
fn containers(dir: &std::path::Path) -> Vec<(&'static str, ArchiveFormat, Vec<u8>)> {
    let tar = tar_of(ENTRIES);
    vec![
        ("zip", ArchiveFormat::Zip, zip_of(ENTRIES)),
        ("tar", ArchiveFormat::Tar, tar.clone()),
        ("tar.gz", ArchiveFormat::Gzip, gzip(&tar)),
        ("tar.bz2", ArchiveFormat::Bzip2, bzip2(&tar)),
        ("tar.xz", ArchiveFormat::Xz, xz(&tar)),
        ("tar.zst", ArchiveFormat::Zstd, zstd(&tar)),
        ("7z", ArchiveFormat::SevenZip, sevenz_of(dir, ENTRIES)),
    ]
}

#[test]
fn every_container_lists_its_entries_with_kinds_and_sizes() {
    let dir = tempfile::tempdir().unwrap();
    for (name, format, bytes) in containers(dir.path()) {
        let path = write(dir.path(), &format!("a.{name}"), &bytes);
        let listing = list(&path, format, ALL, BIG).unwrap_or_else(|e| panic!("{name}: {e}"));
        let seen: Vec<(&str, EntryKind, Option<u64>)> = listing
            .entries
            .iter()
            .map(|e| (e.path.trim_end_matches('/'), e.kind, e.size.map(|s| s.0)))
            .collect();
        let mut want = vec![
            ("docs", EntryKind::Directory, Some(0)),
            ("docs/readme.txt", EntryKind::File, Some(13)),
            ("data.bin", EntryKind::File, Some(10)),
        ];
        if name.starts_with("tar") {
            want.push(("link", EntryKind::Link, Some(0)));
        }
        let mut seen_sorted = seen.clone();
        // 7z keeps its own order; compare as sets of rows.
        seen_sorted.sort_by_key(|row| row.0.to_owned());
        want.sort_by_key(|row| row.0.to_owned());
        assert_eq!(seen_sorted, want, "{name}");
        assert_eq!(listing.holds, Holds::Entries, "{name}");
        assert_eq!(
            listing.count,
            EntryCount::Exact(want.len() as u32),
            "{name}"
        );
        assert_eq!(listing.unpacked, ByteLen(23), "{name}");
    }
}

#[test]
fn a_listing_keeps_the_first_entries_and_counts_them_all() {
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..100).map(|i| format!("f{i:03}.txt")).collect();
    let entries: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
    for (name, format, bytes) in [
        ("zip", ArchiveFormat::Zip, zip_of(&entries)),
        ("tar", ArchiveFormat::Tar, tar_of(&entries)),
    ] {
        let path = write(dir.path(), &format!("many.{name}"), &bytes);
        let listing = list(&path, format, EntryLimit(5), BIG).unwrap();
        let first: Vec<&str> = listing.entries.iter().map(|e| e.path.as_str()).collect();
        let expected = ["f000.txt", "f001.txt", "f002.txt", "f003.txt", "f004.txt"];
        assert_eq!(first, expected, "{name}");
        let total = if name == "tar" { 101 } else { 100 }; // the tar's link
        assert_eq!(listing.count, EntryCount::Exact(total), "{name}");
    }
}

#[test]
fn a_compressed_file_that_is_not_a_tar_is_one_file_named_without_its_extension() {
    let dir = tempfile::tempdir().unwrap();
    let text = "just some words, not a tar at all\n".repeat(30);
    for (name, format, bytes) in [
        ("gz", ArchiveFormat::Gzip, gzip(text.as_bytes())),
        ("bz2", ArchiveFormat::Bzip2, bzip2(text.as_bytes())),
        ("xz", ArchiveFormat::Xz, xz(text.as_bytes())),
        ("zst", ArchiveFormat::Zstd, zstd(text.as_bytes())),
    ] {
        let path = write(dir.path(), &format!("notes.txt.{name}"), &bytes);
        let listing = list(&path, format, ALL, BIG).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(listing.holds, Holds::OneFile, "{name}");
        assert_eq!(listing.entries.len(), 1, "{name}");
        assert_eq!(listing.entries[0].path, "notes.txt", "{name}");
        assert_eq!(
            listing.entries[0].size,
            Some(ByteLen(text.len() as u64)),
            "{name}"
        );
        let got = extract(&path, format, "notes.txt", LIMITS).unwrap();
        assert_eq!(got, text.as_bytes(), "{name}");
    }
}

#[test]
fn a_stream_larger_than_the_budget_lists_what_fits_and_says_there_may_be_more() {
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..200).map(|i| format!("file{i:03}.txt")).collect();
    let entries: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "payload")).collect();
    let path = write(dir.path(), "big.tar.gz", &gzip(&tar_of(&entries)));
    let listing = list(&path, ArchiveFormat::Gzip, ALL, ByteLen(8 * 1024)).unwrap();
    let EntryCount::AtLeast(counted) = listing.count else {
        panic!("a cut stream has a lower bound, not {:?}", listing.count);
    };
    assert!((5..200).contains(&counted), "{counted}");
    assert_eq!(listing.entries[0].path, "file000.txt");
}

#[test]
fn an_index_larger_than_the_budget_is_refused_for_a_zip_and_a_zero_budget_for_all() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.zip", &zip_of(ENTRIES));
    assert_eq!(
        list(&path, ArchiveFormat::Zip, ALL, ByteLen(10)),
        Err(ArchiveError::OverBudget {
            allowed: ByteLen(10)
        })
    );
    assert_eq!(
        list(&path, ArchiveFormat::Zip, ALL, ByteLen(0)),
        Err(ArchiveError::NoBudget)
    );
}

#[test]
fn bytes_that_are_not_the_format_are_malformed() {
    let dir = tempfile::tempdir().unwrap();
    let junk = write(
        dir.path(),
        "junk",
        b"this is not an archive of any kind, not even close",
    );
    for format in [
        ArchiveFormat::Zip,
        ArchiveFormat::Tar,
        ArchiveFormat::SevenZip,
        ArchiveFormat::Gzip,
        ArchiveFormat::Zstd,
        ArchiveFormat::Xz,
        ArchiveFormat::Bzip2,
    ] {
        let got = list(&junk, format, ALL, BIG);
        assert!(
            matches!(got, Err(ArchiveError::Malformed { .. })),
            "{format:?}: {got:?}"
        );
    }
}

#[test]
fn one_entry_comes_out_of_every_container_and_the_wrong_asks_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    for (name, format, bytes) in containers(dir.path()) {
        let path = write(dir.path(), &format!("x.{name}"), &bytes);
        let got = extract(&path, format, "docs/readme.txt", LIMITS).unwrap();
        assert_eq!(got, b"hello archive", "{name}");
        let missing = extract(&path, format, "nope.txt", LIMITS);
        assert!(
            matches!(missing, Err(ArchiveError::NoSuchEntry { .. })),
            "{name}: {missing:?}"
        );
        let small = ExtractLimits {
            entry: ByteLen(5),
            ..LIMITS
        };
        assert_eq!(
            extract(&path, format, "docs/readme.txt", small),
            Err(ArchiveError::TooLarge {
                allowed: ByteLen(5)
            }),
            "{name}"
        );
        let folder_name = if name == "7z" { "docs" } else { "docs/" };
        assert_eq!(
            extract(&path, format, folder_name, LIMITS),
            Err(ArchiveError::NotAFile),
            "{name}"
        );
    }
}

#[test]
fn the_peek_words_what_the_archive_is_and_holds() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = gzip(&tar_of(ENTRIES));
    let path = write_path(dir.path(), "bundle.tar.gz", &bytes);
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    let src = Source::new(path, stamp);
    let SniffStep::Done(sniffed) = sniff(
        &FileHead::new(&bytes[..bytes.len().min(4096)]),
        &FileName::new("bundle.tar.gz").unwrap(),
    ) else {
        panic!("a gzip head is answered at once");
    };
    assert_eq!(sniffed.kind(), FormatKind::Archive);
    let budget = PeekBudget {
        bytes: BIG,
        pixels: PixelArea(1 << 20),
        time: Duration::from_secs(1),
    };
    let peeked = ArchivePeek::peek(&src.on_disk(), &sniffed, &budget).unwrap();
    let facts = ArchivePeek::facts(&peeked);
    assert_eq!(
        facts.value(FactLabel::Kind).map(|v| v.as_str()),
        Some("Tar (gzip)")
    );
    assert_eq!(
        facts.value(FactLabel::Entries).map(|v| v.as_str()),
        Some("4, 23 B unpacked")
    );
    // A peek of a file that is not an archive's kind refuses.
    let text = FileName::new("a.txt").unwrap();
    let SniffStep::Done(plain) = sniff(&FileHead::new(b"hello"), &text) else {
        panic!("text is answered at once");
    };
    assert!(matches!(
        ArchivePeek::peek(&src.on_disk(), &plain, &budget),
        Err(ArchiveError::WrongKind { .. })
    ));
}

#[test]
fn a_zips_names_and_mimetype_are_what_sniffing_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let entries = [
        ("mimetype", "application/epub+zip"),
        ("META-INF/container.xml", "<container/>"),
        ("OEBPS/chapter.xhtml", "<html/>"),
    ];
    let path = write(dir.path(), "novel.epub", &zip_of(&entries));
    assert_eq!(
        zip_entries(&path, BIG).unwrap(),
        ZipEntries::new(
            ["mimetype", "META-INF/container.xml", "OEBPS/chapter.xhtml"],
            Some(b"application/epub+zip".as_slice())
        )
    );
    // Without a mimetype entry there is none to give.
    let plain = write(dir.path(), "plain.zip", &zip_of(ENTRIES));
    assert_eq!(
        zip_entries(&plain, BIG).unwrap(),
        ZipEntries::new(["docs/", "docs/readme.txt", "data.bin"], None)
    );
    assert_eq!(
        zip_entries(&plain, ByteLen(10)),
        Err(ArchiveError::OverBudget {
            allowed: ByteLen(10)
        })
    );
    let junk = write(dir.path(), "junk.zip", b"PK\x03\x04 and then nothing");
    assert!(matches!(
        zip_entries(&junk, BIG),
        Err(ArchiveError::Malformed { .. })
    ));
}
