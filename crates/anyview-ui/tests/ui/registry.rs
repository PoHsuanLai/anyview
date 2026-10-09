//! Every kind of file is mapped to a view, and the registry agrees with the core's table of which
//! kinds have a stage: a kind with no stage is shown as facts, never left out.

use anyview_core::{FormatKind, OfficeFormat, StageSupport, stage_support};
use anyview_ui::{StageFamily, family_of};
use ds_core::word::Word;

#[test]
fn every_kind_lands_on_its_family_and_the_registry_agrees_with_the_core() {
    // name, kind, family
    let mut cases: Vec<(String, FormatKind, StageFamily)> = [
        ("a photo", FormatKind::Raster, StageFamily::Raster),
        ("an svg", FormatKind::Vector, StageFamily::Raster),
        ("a pdf", FormatKind::Pdf, StageFamily::Pdf),
        ("markdown", FormatKind::Markdown, StageFamily::Text),
        ("code", FormatKind::Code, StageFamily::Text),
        ("plain text", FormatKind::PlainText, StageFamily::Text),
        ("a table", FormatKind::Table, StageFamily::Table),
        ("json", FormatKind::Tree, StageFamily::Tree),
        ("a video", FormatKind::Video, StageFamily::Media),
        ("a song", FormatKind::Audio, StageFamily::Media),
        (
            "an office document",
            FormatKind::Office,
            StageFamily::PeekOnly,
        ),
        ("a book", FormatKind::Book, StageFamily::Pdf),
        ("an archive", FormatKind::Archive, StageFamily::PeekOnly),
        ("a font", FormatKind::Font, StageFamily::PeekOnly),
        ("a folder", FormatKind::Folder, StageFamily::PeekOnly),
        ("something else", FormatKind::Other, StageFamily::PeekOnly),
    ]
    .into_iter()
    .map(|(name, kind, family)| (name.to_owned(), kind, family))
    .collect();
    // Spreadsheets are tables and other office files are facts.
    for format in [OfficeFormat::Xlsx, OfficeFormat::Ods, OfficeFormat::Xls] {
        cases.push((format!("{format:?}"), format.kind(), StageFamily::Table));
    }
    for format in [OfficeFormat::Docx, OfficeFormat::Pptx, OfficeFormat::Odt] {
        cases.push((format!("{format:?}"), format.kind(), StageFamily::PeekOnly));
    }
    for kind in FormatKind::ALL {
        assert!(
            cases.iter().any(|(_, k, _)| k == kind),
            "{kind:?} has no row: every kind has a view"
        );
    }
    for (name, kind, family) in &cases {
        assert_eq!(family_of(*kind), *family, "row {name}: its family");
        let shown = match family_of(*kind) {
            StageFamily::PeekOnly => StageSupport::PeekOnly,
            StageFamily::Raster
            | StageFamily::Pdf
            | StageFamily::Media
            | StageFamily::Text
            | StageFamily::Table
            | StageFamily::Tree => StageSupport::Stage,
        };
        assert_eq!(shown, stage_support(*kind), "row {name}: the core agrees");
    }
}
