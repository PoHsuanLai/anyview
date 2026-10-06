//! Loading fixtures the way the viewer meets files: a `Source` on disk and what sniffing made of it.
// Each test crate uses some of these helpers, and none of them is a `#[test]` function.
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{
    ByteLen, FileHead, FileName, FilePath, FileStamp, ModTime, PeekBudget, PixelArea, SniffStep,
    Sniffed, Source, sniff,
};
use std::path::PathBuf;
use std::time::Duration;

/// The path of a fixture.
pub fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A fixture's bytes.
pub fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(path(name)).unwrap()
}

/// The fixture as the viewer would be handed it.
pub fn fixture(name: &str) -> (Source, Sniffed) {
    let bytes = bytes(name);
    let head = &bytes[..bytes.len().min(4096)];
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(head), &FileName::new(name).unwrap())
    else {
        panic!("{name} is not a zip");
    };
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    (
        Source::new(FilePath::new(path(name)).unwrap(), stamp),
        sniffed,
    )
}

/// A budget that reads `bytes` bytes.
pub fn budget(bytes: u64) -> PeekBudget {
    PeekBudget {
        bytes: ByteLen(bytes),
        pixels: PixelArea(0),
        time: Duration::from_millis(500),
    }
}

/// The facts as `(label slug, text)` rows, in order.
pub fn rows(facts: &anyview_core::Facts) -> Vec<(&'static str, String)> {
    use ds_core::word::Word;
    facts
        .rows()
        .iter()
        .map(|fact| (fact.label.slug(), fact.value.as_str().to_owned()))
        .collect()
}

/// `rows` written as a literal.
pub fn expected(rows: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    rows.iter().map(|(l, v)| (*l, (*v).to_owned())).collect()
}

/// A workbook whose sheets are named `sheets` and whose `<sheetData>` contents are `sheet_data`,
/// written into `dir` as `name`, and the viewer's view of it.
pub fn workbook_of_xml(
    dir: &std::path::Path,
    name: &str,
    sheets: &[&str],
    sheet_data: &[String],
) -> (Source, Sniffed) {
    use std::io::Write;
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let mut put = |entry: &str, body: String| {
        zip.start_file(entry, options).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    };
    let overrides: String = (1..=sheets.len())
        .map(|n| format!(r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
        .collect();
    put(
        "[Content_Types].xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>{overrides}</Types>"#
        ),
    );
    put("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_owned());
    let listed: String = sheets
        .iter()
        .enumerate()
        .map(|(at, sheet)| {
            format!(
                r#"<sheet name="{sheet}" sheetId="{}" r:id="rId{}"/>"#,
                at + 1,
                at + 1
            )
        })
        .collect();
    put(
        "xl/workbook.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{listed}</sheets></workbook>"#
        ),
    );
    let rels: String = (1..=sheets.len())
        .map(|n| format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#))
        .collect();
    put(
        "xl/_rels/workbook.xml.rels",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
        ),
    );
    for (at, data) in sheet_data.iter().enumerate() {
        put(
            &format!("xl/worksheets/sheet{}.xml", at + 1),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{data}</sheetData></worksheet>"#
            ),
        );
    }
    zip.finish().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    let SniffStep::LookInside(probe) =
        sniff(&FileHead::new(&bytes[..4]), &FileName::new(name).unwrap())
    else {
        panic!("a zip's head asks to be looked inside");
    };
    let sniffed = anyview_core::sniff_zip(
        probe,
        &anyview_core::ZipEntries::new(["[Content_Types].xml", "xl/workbook.xml"], None),
    );
    (Source::new(FilePath::new(&path).unwrap(), stamp), sniffed)
}

/// A workbook of `sheets` (a name and its rows of text cells; a cell that parses as a number is
/// stored as one) written into `dir` as `name`, and the viewer's view of it.
pub fn workbook(
    dir: &std::path::Path,
    name: &str,
    sheets: &[(&str, &[&[&str]])],
) -> (Source, Sniffed) {
    let names: Vec<&str> = sheets.iter().map(|(sheet, _)| *sheet).collect();
    let data: Vec<String> = sheets
        .iter()
        .map(|(_, rows)| {
            rows.iter()
                .enumerate()
                .map(|(r, row)| {
                    let cells: String = row
                        .iter()
                        .enumerate()
                        .map(|(c, text)| {
                            let at = format!("{}{}", (b'A' + c as u8) as char, r + 1);
                            match text.parse::<f64>() {
                                Ok(_) => format!(r#"<c r="{at}"><v>{text}</v></c>"#),
                                Err(_) => {
                                    format!(
                                        r#"<c r="{at}" t="inlineStr"><is><t>{text}</t></is></c>"#
                                    )
                                }
                            }
                        })
                        .collect();
                    format!(r#"<row r="{}">{cells}</row>"#, r + 1)
                })
                .collect()
        })
        .collect();
    workbook_of_xml(dir, name, &names, &data)
}

/// A file of `bytes` in `dir`, as the viewer would be handed it.
pub fn written(dir: &std::path::Path, name: &str, bytes: &[u8]) -> (Source, Sniffed) {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    let head = &bytes[..bytes.len().min(4096)];
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(head), &FileName::new(name).unwrap())
    else {
        panic!("{name} is not a zip");
    };
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(0),
    };
    (Source::new(FilePath::new(&path).unwrap(), stamp), sniffed)
}
