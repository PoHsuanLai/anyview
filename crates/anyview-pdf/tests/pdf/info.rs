//! `PdfDocument::info`: the Info dictionary, the XMP packet, protection, tagging, attachments and
//! signatures of PDFs built in memory.

use anyview_core::{FactGroup, FactLabel, FactValue, Facts, LocalZone};
use anyview_pdf::{
    FormatVersion, Orientation, PageFormat, PdfDocument, PdfInfo, Protection, Restriction, Tagging,
};
use pdfrum::{Document, Permissions};
use pdfrum_edit::{EditDoc, Encryption, SaveOptions, save};

const XMP: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">The XMP Title</rdf:li></rdf:Alt></dc:title>
<dc:creator><rdf:Seq><rdf:li>XMP Author</rdf:li></rdf:Seq></dc:creator>
</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;

/// A PDF of `objects` (numbered from 1; object 1 is the catalog), with a correct cross-reference
/// table. `trailer` is added to the trailer dictionary.
fn build(objects: &[String], trailer: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R {trailer} >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn stream(dict: &str, data: &str) -> String {
    format!(
        "<< {dict} /Length {} >>\nstream\n{data}\nendstream",
        data.len()
    )
}

/// An A4 document with a full Info dictionary and an XMP packet that disagrees with it, tagged,
/// with one attachment and one signature field.
fn described() -> Vec<u8> {
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R /Metadata 4 0 R /MarkInfo << /Marked true >> \
         /Names << /EmbeddedFiles << /Names [(notes.txt) 5 0 R] >> >> \
         /AcroForm << /Fields [7 0 R] /SigFlags 3 >> >>"
            .into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] >>".into(),
        stream("/Type /Metadata /Subtype /XML", XMP),
        "<< /Type /Filespec /F (notes.txt) /EF << /F 6 0 R >> >>".into(),
        stream("/Type /EmbeddedFile", "hello"),
        "<< /FT /Sig /T (Signature1) /V 8 0 R >>".into(),
        "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached \
         /ByteRange [0 10 20 10] /Contents <00> /M (D:20261001120000Z) >>"
            .into(),
        "<< /Title (Quarterly Report) /Author (Ada Lovelace) /Subject (Numbers) \
         /Keywords (finance, q3) /Creator (Writer) /Producer (TestPress 1.2) \
         /CreationDate (D:20261009140500+02'00') /ModDate (D:20261010081500Z) >>"
            .into(),
    ];
    build(&objects, "/Info 9 0 R")
}

/// A Letter document with no Info dictionary at all, only the XMP packet.
fn xmp_only() -> Vec<u8> {
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R /Metadata 4 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".into(),
        stream("/Type /Metadata /Subtype /XML", XMP),
    ];
    build(&objects, "")
}

fn plain() -> Vec<u8> {
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] >>".into(),
    ];
    build(&objects, "")
}

fn info_of(bytes: Vec<u8>) -> PdfInfo {
    PdfDocument::from_bytes(bytes).unwrap().info()
}

fn row(facts: &Facts, label: FactLabel) -> Option<&str> {
    facts.value(label).map(FactValue::as_str)
}

#[test]
fn the_info_dictionary_gives_the_document_section() {
    let info = info_of(described());
    assert_eq!(info.title.as_deref(), Some("Quarterly Report"));
    assert_eq!(info.author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(info.version, Some(FormatVersion { major: 1, minor: 7 }));
    assert_eq!(
        info.page_size,
        PageFormat::Named {
            name: "A4",
            orientation: Orientation::Portrait
        }
    );
    assert_eq!(info.protection, Protection::Open);
    assert_eq!(info.tagging, Tagging::Tagged);
    assert_eq!(info.attachments, 1);
    assert_eq!(info.signatures, 1);

    // Two hours east of Greenwich: the first date is already there, the second is two hours on.
    let facts = info.facts_in(&LocalZone::fixed(120));
    assert!(facts.rows().iter().all(|r| r.group == FactGroup::Document));
    let got: Vec<(FactLabel, &str)> = facts
        .rows()
        .iter()
        .map(|r| (r.label, r.value.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (FactLabel::Title, "Quarterly Report"),
            (FactLabel::Author, "Ada Lovelace"),
            (FactLabel::Subject, "Numbers"),
            (FactLabel::Keywords, "finance, q3"),
            (FactLabel::Created, "9 Oct 2026 at 14:05"),
            (FactLabel::Modified, "10 Oct 2026 at 10:15"),
            (FactLabel::Creator, "Writer"),
            (FactLabel::Producer, "TestPress 1.2"),
            (FactLabel::Version, "1.7"),
            (FactLabel::PageSize, "A4"),
            (FactLabel::Pages, "1 page"),
            (FactLabel::Tagged, "Yes"),
            (FactLabel::Attachments, "1 file"),
            (FactLabel::Signatures, "1 signature"),
        ]
    );
}

#[test]
fn the_xmp_packet_stands_in_for_a_missing_info_dictionary() {
    let info = info_of(xmp_only());
    assert_eq!(info.title.as_deref(), Some("The XMP Title"));
    assert_eq!(info.author.as_deref(), Some("XMP Author"));
    assert_eq!(info.created, None);
    assert_eq!(info.tagging, Tagging::Untagged);
    let facts = info.facts();
    assert_eq!(row(&facts, FactLabel::Title), Some("The XMP Title"));
    assert_eq!(row(&facts, FactLabel::PageSize), Some("Letter"));
    for absent in [
        FactLabel::Created,
        FactLabel::Security,
        FactLabel::Tagged,
        FactLabel::Attachments,
        FactLabel::Signatures,
    ] {
        assert_eq!(row(&facts, absent), None, "{absent:?}");
    }
}

#[test]
fn a_file_with_no_metadata_still_has_its_pages_and_size() {
    let info = info_of(plain());
    assert_eq!(
        (&info.title, &info.author, &info.created),
        (&None, &None, &None)
    );
    assert_eq!(info.page_size, PageFormat::Varies);
    let facts = info.facts();
    assert_eq!(row(&facts, FactLabel::Pages), Some("2 pages"));
    assert_eq!(row(&facts, FactLabel::PageSize), Some("Varies"));
    assert_eq!(row(&facts, FactLabel::Version), Some("1.7"));
    assert_eq!(row(&facts, FactLabel::Title), None);
}

#[test]
fn an_encrypted_file_that_opens_names_what_the_owner_took_away() {
    let encryption = Encryption::builder()
        .owner_password(b"owner".to_vec())
        .permissions(Permissions {
            print: false,
            copy: false,
            ..Permissions::ALL
        })
        .build();
    let mut bytes = Vec::new();
    let original = Document::from_bytes(plain()).unwrap();
    let edit = EditDoc::new(original.parser());
    save(
        &edit,
        &SaveOptions::builder().encrypt(encryption).build(),
        &mut bytes,
    )
    .unwrap();
    let info = info_of(bytes);
    assert_eq!(
        info.protection,
        Protection::Encrypted(vec![Restriction::Printing, Restriction::Copying])
    );
    assert_eq!(
        row(&info.facts(), FactLabel::Security),
        Some("Encrypted (printing and copying not allowed)")
    );
}

/// An A4 document whose Info dictionary is `info` and whose XMP packet is `xmp`, if any.
fn with_info(info: &str, xmp: Option<&str>) -> Vec<u8> {
    let catalog = if xmp.is_some() {
        "<< /Type /Catalog /Pages 2 0 R /Metadata 4 0 R >>"
    } else {
        "<< /Type /Catalog /Pages 2 0 R >>"
    };
    let mut objects = vec![
        catalog.into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] >>".into(),
        stream("/Type /Metadata /Subtype /XML", xmp.unwrap_or("")),
        info.to_owned(),
    ];
    if xmp.is_none() {
        objects.remove(3);
    }
    let info_object = objects.len();
    build(&objects, &format!("/Info {info_object} 0 R"))
}

const XMP_DATES: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:CreateDate="2026-10-09T14:05:00+02:00" xmp:ModifyDate="2026-10-10T08:15:00Z">
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">The XMP Title</rdf:li></rdf:Alt></dc:title>
<dc:creator><rdf:Seq><rdf:li>XMP Author</rdf:li></rdf:Seq></dc:creator>
</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;

#[test]
fn empty_or_blank_info_strings_do_not_hide_the_xmp_packet() {
    for (name, info) in [
        ("empty", "<< /Title () /Author () >>"),
        ("blank", "<< /Title (   ) /Author (\t\n ) >>"),
    ] {
        let info = info_of(with_info(info, Some(XMP_DATES)));
        assert_eq!(info.title.as_deref(), Some("The XMP Title"), "{name}");
        assert_eq!(info.author.as_deref(), Some("XMP Author"), "{name}");
    }
    let info = info_of(with_info("<< /Title () /Creator ( ) /Producer () >>", None));
    assert_eq!(
        (&info.title, &info.creator, &info.producer),
        (&None, &None, &None)
    );
    assert_eq!(row(&info.facts(), FactLabel::Title), None);
}

#[test]
fn titles_in_utf16_and_pdfdoc_encoding_read_as_text() {
    // "Café ☕" as UTF-16BE with a byte order mark.
    let utf16: String = "\u{FEFF}Café ☕"
        .encode_utf16()
        .map(|unit| format!("{unit:04X}"))
        .collect();
    let info = info_of(with_info(&format!("<< /Title <{utf16}> >>"), None));
    assert_eq!(info.title.as_deref(), Some("Café ☕"));
    // PDFDocEncoding: 0x8D and 0x8E are the curly double quotes, 0xE9 is é.
    let info = info_of(with_info("<< /Title (\\215Caf\\351\\216) >>", None));
    assert_eq!(info.title.as_deref(), Some("\u{201C}Café\u{201D}"));
}

#[test]
fn iso_style_info_dates_read_and_a_bad_one_falls_back_to_the_xmp_packet() {
    let zone = LocalZone::fixed(0);
    let info = info_of(with_info(
        "<< /CreationDate (D:2026-10-09) /ModDate (D:2026-10-10T08:15:00Z) >>",
        None,
    ));
    let facts = info.facts_in(&zone);
    assert_eq!(row(&facts, FactLabel::Created), Some("9 Oct 2026"));
    assert_eq!(
        row(&facts, FactLabel::Modified),
        Some("10 Oct 2026 at 08:15")
    );
    // An Info date that is not a date, or none at all: the packet's dates stand in, converted.
    for (name, info) in [
        ("garbage", "<< /CreationDate (yesterday) /ModDate (soon) >>"),
        ("absent", "<< /Title (T) >>"),
        ("empty", "<< /CreationDate () /ModDate ( ) >>"),
    ] {
        let info = info_of(with_info(info, Some(XMP_DATES)));
        let facts = info.facts_in(&zone);
        assert_eq!(
            row(&facts, FactLabel::Created),
            Some("9 Oct 2026 at 12:05"),
            "{name}"
        );
        assert_eq!(
            row(&facts, FactLabel::Modified),
            Some("10 Oct 2026 at 08:15"),
            "{name}"
        );
    }
    // The Info dictionary's own date wins when it parses.
    let info = info_of(with_info(
        "<< /CreationDate (D:20200101000000Z) >>",
        Some(XMP_DATES),
    ));
    assert_eq!(
        row(&info.facts_in(&zone), FactLabel::Created),
        Some("1 Jan 2020 at 00:00")
    );
}

#[test]
fn pdf_dates_are_shown_in_the_persons_zone() {
    let info = info_of(described());
    // 14:05 at +02:00 is 11:05 in Brazil (UTC-3); 08:15 UTC is 05:15 there.
    let facts = info.facts_in(&LocalZone::fixed(-180));
    assert_eq!(row(&facts, FactLabel::Created), Some("9 Oct 2026 at 09:05"));
    assert_eq!(
        row(&facts, FactLabel::Modified),
        Some("10 Oct 2026 at 05:15")
    );
}
