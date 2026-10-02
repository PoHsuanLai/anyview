//! Every kind of file is mapped to a view, and the registry agrees with the core's table of which
//! kinds have a stage: a kind with no stage is shown as facts and Open With…, never left out.

use anyview_core::{FormatKind, StageSupport, stage_support};
use anyview_ui::{StageFamily, family_of};
use ds_core::word::Word;

#[test]
fn every_kind_has_a_view_and_the_registry_agrees_with_the_core() {
    for kind in FormatKind::ALL {
        let shown = match family_of(*kind) {
            StageFamily::PeekOnly => StageSupport::PeekOnly,
            StageFamily::Raster | StageFamily::Pdf | StageFamily::Media | StageFamily::Text => {
                StageSupport::Stage
            }
        };
        assert_eq!(shown, stage_support(*kind), "{kind:?}");
    }
}

#[test]
fn the_kinds_the_viewer_shows_land_on_their_families() {
    // name, kind, family
    const CASES: &[(&str, FormatKind, StageFamily)] = &[
        ("a photo", FormatKind::Raster, StageFamily::Raster),
        ("an svg", FormatKind::Vector, StageFamily::Raster),
        ("a pdf", FormatKind::Pdf, StageFamily::Pdf),
        ("markdown", FormatKind::Markdown, StageFamily::Text),
        ("code", FormatKind::Code, StageFamily::Text),
        ("plain text", FormatKind::PlainText, StageFamily::Text),
        ("a table", FormatKind::Table, StageFamily::Text),
        ("json", FormatKind::Tree, StageFamily::Text),
        ("a video", FormatKind::Video, StageFamily::Media),
        ("a song", FormatKind::Audio, StageFamily::Media),
        ("an archive", FormatKind::Archive, StageFamily::PeekOnly),
        ("a folder", FormatKind::Folder, StageFamily::PeekOnly),
        ("something else", FormatKind::Other, StageFamily::PeekOnly),
    ];
    for (name, kind, family) in CASES {
        assert_eq!(family_of(*kind), *family, "{name}");
    }
}
