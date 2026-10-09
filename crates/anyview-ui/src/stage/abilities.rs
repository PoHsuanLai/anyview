//! What each family of stage can do to its file, declared once. The menu, the palette, the
//! capsule and the keys all list from what the showing stage declares here (with the kind's
//! actions and the platform's services), so nothing is offered that the stage cannot carry out.

use super::model::Stage;
use crate::sheet::ExportFamily;
use anyview_core::FileAction;

/// The file actions that change what the stage shows, the export it writes, and whether it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageAbilities {
    /// The actions that rewrite the file in place, each of which the stage does.
    pub edits: &'static [FileAction],
    /// The format the Export sheet writes, or `None` when the stage has no export.
    pub export: Option<ExportFamily>,
    /// Whether the file plays: Play in a Small Window and Play in the Background are its.
    pub plays: bool,
}

/// A stage that changes nothing and writes nothing.
const READ_ONLY: StageAbilities = StageAbilities {
    edits: &[],
    export: None,
    plays: false,
};

/// A picture turns and flips.
const RASTER: StageAbilities = StageAbilities {
    edits: &[
        FileAction::RotateLeft,
        FileAction::RotateRight,
        FileAction::FlipHorizontal,
        FileAction::FlipVertical,
    ],
    export: Some(ExportFamily::Raster),
    plays: false,
};

/// A PDF turns the page the person is on (the file stores it as the page's `/Rotate`), and does
/// not flip.
const PDF: StageAbilities = StageAbilities {
    edits: &[FileAction::RotateLeft, FileAction::RotateRight],
    export: Some(ExportFamily::Pdf),
    plays: false,
};

const MEDIA: StageAbilities = StageAbilities {
    export: Some(ExportFamily::Media),
    plays: true,
    ..READ_ONLY
};

const TEXT: StageAbilities = StageAbilities {
    export: Some(ExportFamily::Text),
    ..READ_ONLY
};

impl Stage {
    /// What this stage can do. The one match on the stage for the file actions it carries out.
    pub fn abilities(&self) -> StageAbilities {
        match self {
            Stage::Raster(_) => RASTER,
            Stage::Pdf(_) => PDF,
            Stage::Media(_) => MEDIA,
            Stage::Text(_) => TEXT,
            Stage::NoStage | Stage::Book(_) | Stage::Table(_) | Stage::Tree(_) => READ_ONLY,
        }
    }
}
