//! Where each office format keeps what the facts are read from: one table, one match on the
//! format.

use super::xml::{attribute, count_elements, element_text};
use super::{OfficeCount, ThumbnailCodec};
use anyview_core::OfficeFormat;

/// What one metadata part gave.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub count: Option<OfficeCount>,
}

/// How a metadata part is read.
#[derive(Debug, Clone, Copy)]
pub(super) enum Reading {
    /// `docProps/core.xml`: Dublin Core title and creator.
    CoreProperties,
    /// `docProps/app.xml` of a document: its page count.
    PageCount,
    /// `docProps/app.xml` of a presentation: its slide count.
    SlideCount,
    /// `xl/workbook.xml`: one `sheet` element per sheet.
    SheetList,
    /// OpenDocument `meta.xml`, with the statistic its count is read from.
    DocumentMeta(Statistic),
}

/// Which attribute of `meta:document-statistic` counts a document.
#[derive(Debug, Clone, Copy)]
pub(super) enum Statistic {
    /// `meta:page-count` of a text document.
    Pages,
    /// `meta:page-count` of a presentation: its slides.
    Slides,
    /// `meta:table-count` of a spreadsheet.
    Sheets,
    /// A drawing has no count.
    Nothing,
}

/// A metadata part of a package.
#[derive(Debug, Clone, Copy)]
pub(super) struct Source {
    pub name: &'static str,
    reading: Reading,
}

fn number(text: Option<String>) -> Option<u32> {
    text.and_then(|text| text.trim().parse().ok())
}

impl Source {
    /// What this part's bytes say.
    pub(super) fn read(&self, xml: &[u8]) -> Metadata {
        let counted = |count: Option<OfficeCount>| count;
        match self.reading {
            Reading::CoreProperties => Metadata {
                title: element_text(xml, "title"),
                author: element_text(xml, "creator"),
                count: None,
            },
            Reading::PageCount => Metadata {
                count: counted(number(element_text(xml, "Pages")).map(OfficeCount::Pages)),
                ..Metadata::default()
            },
            Reading::SlideCount => Metadata {
                count: counted(number(element_text(xml, "Slides")).map(OfficeCount::Slides)),
                ..Metadata::default()
            },
            Reading::SheetList => Metadata {
                count: counted(Some(OfficeCount::Sheets(count_elements(xml, "sheet")))),
                ..Metadata::default()
            },
            Reading::DocumentMeta(statistic) => {
                let counted_by = |attr: &str| number(attribute(xml, "document-statistic", attr));
                let count = match statistic {
                    Statistic::Pages => counted_by("page-count").map(OfficeCount::Pages),
                    Statistic::Slides => counted_by("page-count").map(OfficeCount::Slides),
                    Statistic::Sheets => counted_by("table-count").map(OfficeCount::Sheets),
                    Statistic::Nothing => None,
                };
                Metadata {
                    title: element_text(xml, "title"),
                    author: element_text(xml, "initial-creator")
                        .or_else(|| element_text(xml, "creator")),
                    count: counted(count),
                }
            }
        }
    }
}

/// The parts of one package: where its facts are and where its picture may be.
#[derive(Debug, Clone, Copy)]
pub(super) struct Parts {
    pub metadata: &'static [Source],
    pub thumbnails: &'static [(&'static str, ThumbnailCodec)],
}

const fn source(name: &'static str, reading: Reading) -> Source {
    Source { name, reading }
}

const OPEN_XML_THUMBNAILS: &[(&str, ThumbnailCodec)] = &[
    ("docProps/thumbnail.jpeg", ThumbnailCodec::Jpeg),
    ("docProps/thumbnail.jpg", ThumbnailCodec::Jpeg),
    ("docProps/thumbnail.png", ThumbnailCodec::Png),
];
const OPEN_DOCUMENT_THUMBNAILS: &[(&str, ThumbnailCodec)] =
    &[("Thumbnails/thumbnail.png", ThumbnailCodec::Png)];
const IWORK_THUMBNAILS: &[(&str, ThumbnailCodec)] = &[
    ("preview.jpg", ThumbnailCodec::Jpeg),
    ("QuickLook/Thumbnail.jpg", ThumbnailCodec::Jpeg),
];

const META_PAGES: &[Source] = &[source("meta.xml", Reading::DocumentMeta(Statistic::Pages))];
const META_SLIDES: &[Source] = &[source("meta.xml", Reading::DocumentMeta(Statistic::Slides))];
const META_SHEETS: &[Source] = &[source("meta.xml", Reading::DocumentMeta(Statistic::Sheets))];
const META_NONE: &[Source] = &[source(
    "meta.xml",
    Reading::DocumentMeta(Statistic::Nothing),
)];

const DOCX_META: &[Source] = &[CORE, source("docProps/app.xml", Reading::PageCount)];
const PPTX_META: &[Source] = &[CORE, source("docProps/app.xml", Reading::SlideCount)];
const XLSX_META: &[Source] = &[CORE, source("xl/workbook.xml", Reading::SheetList)];

const CORE: Source = source("docProps/core.xml", Reading::CoreProperties);

/// The parts of `format`'s package, or `None` for a format that is not a zip package.
pub(super) fn parts_of(format: OfficeFormat) -> Option<Parts> {
    let open_xml = |metadata: &'static [Source]| Parts {
        metadata,
        thumbnails: OPEN_XML_THUMBNAILS,
    };
    let open_document = |metadata: &'static [Source]| Parts {
        metadata,
        thumbnails: OPEN_DOCUMENT_THUMBNAILS,
    };
    match format {
        OfficeFormat::Docx => Some(open_xml(DOCX_META)),
        OfficeFormat::Pptx => Some(open_xml(PPTX_META)),
        OfficeFormat::Xlsx => Some(open_xml(XLSX_META)),
        OfficeFormat::Odt => Some(open_document(META_PAGES)),
        OfficeFormat::Odp => Some(open_document(META_SLIDES)),
        OfficeFormat::Ods => Some(open_document(META_SHEETS)),
        OfficeFormat::Odg => Some(open_document(META_NONE)),
        OfficeFormat::Pages | OfficeFormat::Numbers | OfficeFormat::Keynote => Some(Parts {
            metadata: &[],
            thumbnails: IWORK_THUMBNAILS,
        }),
        OfficeFormat::Doc | OfficeFormat::Xls | OfficeFormat::Ppt => None,
    }
}
