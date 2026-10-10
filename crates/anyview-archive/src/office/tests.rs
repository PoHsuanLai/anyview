use super::*;
use anyview_core::FilePath;
use anyview_fs::OnDisk;
use std::io::Write;

/// A package of `entries` written into a scratch directory.
fn package(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, Input) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, body) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
    let path = FilePath::new(&path).unwrap().on_disk();
    (dir, path)
}

const CORE: &[u8] = br#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="c" xmlns:dc="d"><dc:title>Q3 &amp; plans</dc:title><dc:creator>Ann Author</dc:creator></cp:coreProperties>"#;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nbytes";

/// name, format, entries, wanted
type Case<'a> = (&'a str, OfficeFormat, Vec<(&'a str, &'a [u8])>, OfficeLook);

#[test]
fn each_format_is_read_from_its_own_parts() {
    // name, format, entries, wanted
    let cases: Vec<Case> = vec![
        (
            "docx: core, pages and a jpeg",
            OfficeFormat::Docx,
            vec![
                ("docProps/core.xml", CORE),
                (
                    "docProps/app.xml",
                    br#"<Properties><Pages>12</Pages><Words>9</Words></Properties>"#,
                ),
                ("docProps/thumbnail.jpeg", b"\xFF\xD8jpeg"),
            ],
            OfficeLook {
                title: Some("Q3 & plans".into()),
                author: Some("Ann Author".into()),
                count: Some(OfficeCount::Pages(12)),
                thumbnail: Some(Thumbnail {
                    bytes: b"\xFF\xD8jpeg".to_vec(),
                    codec: ThumbnailCodec::Jpeg,
                }),
            },
        ),
        (
            "pptx: slides",
            OfficeFormat::Pptx,
            vec![
                ("docProps/core.xml", CORE),
                ("docProps/app.xml", br#"<Properties><Slides>7</Slides></Properties>"#),
            ],
            OfficeLook {
                title: Some("Q3 & plans".into()),
                author: Some("Ann Author".into()),
                count: Some(OfficeCount::Slides(7)),
                thumbnail: None,
            },
        ),
        (
            "xlsx: sheets counted from the workbook",
            OfficeFormat::Xlsx,
            vec![(
                "xl/workbook.xml",
                br#"<workbook><sheets><sheet name="a"/><sheet name="b"/></sheets></workbook>"#,
            )],
            OfficeLook {
                count: Some(OfficeCount::Sheets(2)),
                ..OfficeLook::default()
            },
        ),
        (
            "odt: meta and its png",
            OfficeFormat::Odt,
            vec![
                (
                    "meta.xml",
                    br#"<office:document-meta xmlns:office="o" xmlns:meta="m" xmlns:dc="d"><office:meta><dc:title>Letter</dc:title><meta:initial-creator>Bo</meta:initial-creator><dc:creator>Cy</dc:creator><meta:document-statistic meta:page-count="3"/></office:meta></office:document-meta>"#,
                ),
                ("Thumbnails/thumbnail.png", PNG),
            ],
            OfficeLook {
                title: Some("Letter".into()),
                author: Some("Bo".into()),
                count: Some(OfficeCount::Pages(3)),
                thumbnail: Some(Thumbnail {
                    bytes: PNG.to_vec(),
                    codec: ThumbnailCodec::Png,
                }),
            },
        ),
        (
            "ods: tables",
            OfficeFormat::Ods,
            vec![(
                "meta.xml",
                br#"<m xmlns:meta="m"><meta:document-statistic meta:table-count="4"/></m>"#,
            )],
            OfficeLook {
                count: Some(OfficeCount::Sheets(4)),
                ..OfficeLook::default()
            },
        ),
        (
            "keynote: the preview",
            OfficeFormat::Keynote,
            vec![("preview.jpg", b"\xFF\xD8k")],
            OfficeLook {
                thumbnail: Some(Thumbnail {
                    bytes: b"\xFF\xD8k".to_vec(),
                    codec: ThumbnailCodec::Jpeg,
                }),
                ..OfficeLook::default()
            },
        ),
        (
            "a package with none of the parts",
            OfficeFormat::Docx,
            vec![("word/document.xml", b"<w/>")],
            OfficeLook::default(),
        ),
        (
            "a part that is not xml",
            OfficeFormat::Docx,
            vec![("docProps/core.xml", b"\0\0 not xml <<")],
            OfficeLook::default(),
        ),
    ];
    for (name, format, entries, want) in cases {
        let (_dir, path) = package(&entries);
        assert_eq!(office_look(&path, format).unwrap(), want, "{name}");
    }
}

#[test]
fn a_binary_office_format_says_nothing_and_is_not_opened() {
    let path = FilePath::new("/nonexistent/old.doc").unwrap().on_disk();
    assert_eq!(
        office_look(&path, OfficeFormat::Doc).unwrap(),
        OfficeLook::default()
    );
}

#[test]
fn the_facts_list_what_the_document_has_in_order() {
    let look = OfficeLook {
        title: Some("T".into()),
        author: Some("A".into()),
        count: Some(OfficeCount::Pages(1)),
        thumbnail: None,
    };
    let facts = look.facts();
    let rows: Vec<(FactLabel, &str)> = facts
        .rows()
        .iter()
        .map(|row| (row.label, row.value.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (FactLabel::Title, "T"),
            (FactLabel::Author, "A"),
            (FactLabel::Pages, "1 page"),
        ]
    );
    assert!(OfficeLook::default().facts().rows().is_empty());
}

#[test]
fn a_long_title_and_author_are_cut_to_one_line() {
    let long = "w ".repeat(400_000);
    let core = format!(
        "<cp:coreProperties xmlns:cp=\"c\" xmlns:dc=\"d\"><dc:title>{long}</dc:title><dc:creator>Ann\n  Author</dc:creator></cp:coreProperties>"
    );
    let (_dir, path) = package(&[("docProps/core.xml", core.as_bytes())]);
    let look = office_look(&path, OfficeFormat::Docx).unwrap();
    let title = look.title.unwrap();
    // The cut text, without a trailing space, and its ellipsis.
    assert!((NAME_CHARS - 1..=NAME_CHARS + 1).contains(&title.chars().count()));
    assert!(title.ends_with('…'));
    assert_eq!(look.author.as_deref(), Some("Ann Author"));
}
