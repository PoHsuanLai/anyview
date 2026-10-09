//! Office and iWork documents: shown as facts and handed to another program.

use super::FormatKind;
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

impl OfficeFormat {
    /// The kind a file of this format is: a spreadsheet is a table the viewer shows its sheets
    /// of, every other format is an office document shown as facts.
    pub fn kind(self) -> FormatKind {
        match self {
            OfficeFormat::Xlsx | OfficeFormat::Ods | OfficeFormat::Xls => FormatKind::Table,
            OfficeFormat::Docx
            | OfficeFormat::Pptx
            | OfficeFormat::Odt
            | OfficeFormat::Odp
            | OfficeFormat::Odg
            | OfficeFormat::Pages
            | OfficeFormat::Numbers
            | OfficeFormat::Keynote
            | OfficeFormat::Doc
            | OfficeFormat::Ppt => FormatKind::Office,
        }
    }
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
    fn only_the_spreadsheets_are_tables() {
        // name, format, kind
        const CASES: &[(&str, OfficeFormat, FormatKind)] = &[
            ("xlsx", OfficeFormat::Xlsx, FormatKind::Table),
            ("ods", OfficeFormat::Ods, FormatKind::Table),
            ("xls", OfficeFormat::Xls, FormatKind::Table),
            (
                "numbers is iWork",
                OfficeFormat::Numbers,
                FormatKind::Office,
            ),
            ("docx", OfficeFormat::Docx, FormatKind::Office),
            ("pptx", OfficeFormat::Pptx, FormatKind::Office),
            ("odp", OfficeFormat::Odp, FormatKind::Office),
        ];
        for (name, format, kind) in CASES {
            assert_eq!(format.kind(), *kind, "{name}");
        }
    }
}
