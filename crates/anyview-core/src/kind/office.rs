//! Office and iWork documents: shown as facts and handed to another program.

use super::family::Family;
use ds_core::word::Word;

/// An office document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum OfficeFormat {
    /// Word, Open XML.
    Docx,
    /// Excel, Open XML.
    Xlsx,
    /// PowerPoint, Open XML.
    Pptx,
    /// OpenDocument text.
    Odt,
    /// OpenDocument spreadsheet.
    Ods,
    /// OpenDocument presentation.
    Odp,
    /// OpenDocument graphics.
    Odg,
    /// Apple Pages.
    Pages,
    /// Apple Numbers.
    Numbers,
    /// Apple Keynote.
    Keynote,
    /// Word, the binary format.
    Doc,
    /// Excel, the binary format.
    Xls,
    /// PowerPoint, the binary format.
    Ppt,
}

impl Family for OfficeFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            OfficeFormat::Docx => &["docx"],
            OfficeFormat::Xlsx => &["xlsx"],
            OfficeFormat::Pptx => &["pptx"],
            OfficeFormat::Odt => &["odt"],
            OfficeFormat::Ods => &["ods"],
            OfficeFormat::Odp => &["odp"],
            OfficeFormat::Odg => &["odg"],
            OfficeFormat::Pages => &["pages"],
            OfficeFormat::Numbers => &["numbers"],
            OfficeFormat::Keynote => &["key"],
            OfficeFormat::Doc => &["doc"],
            OfficeFormat::Xls => &["xls"],
            OfficeFormat::Ppt => &["ppt"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            OfficeFormat::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            OfficeFormat::Xlsx => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            }
            OfficeFormat::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            OfficeFormat::Odt => "application/vnd.oasis.opendocument.text",
            OfficeFormat::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            OfficeFormat::Odp => "application/vnd.oasis.opendocument.presentation",
            OfficeFormat::Odg => "application/vnd.oasis.opendocument.graphics",
            OfficeFormat::Pages => "application/vnd.apple.pages",
            OfficeFormat::Numbers => "application/vnd.apple.numbers",
            OfficeFormat::Keynote => "application/vnd.apple.keynote",
            OfficeFormat::Doc => "application/msword",
            OfficeFormat::Xls => "application/vnd.ms-excel",
            OfficeFormat::Ppt => "application/vnd.ms-powerpoint",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_well_formed() {
        crate::kind::family::tests::assert_well_formed::<OfficeFormat>();
    }
}
