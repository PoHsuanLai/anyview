//! What each family of stage can do to its file, declared once. The menu, the palette, the
//! capsule and the keys all list from what the showing stage declares here (with the kind's
//! actions and the platform's services), so nothing is offered that the stage cannot carry out.

use super::model::Stage;
use crate::sheet::ExportFamily;
use anyview_core::FileAction;

/// Whether the file a stage shows plays, as a recording does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
    /// It plays: Play in a Small Window and Play in the Background are its.
    Plays,
    /// It does not; it stays as it is.
    Still,
}

/// Whether the file a stage shows can be written on: its form filled, marks made on its pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writing {
    /// It can, if the document lets the person: the Markup tool and its commands are offered.
    Writable,
    /// It cannot.
    Fixed,
}

/// The file actions that change what the stage shows, the export it writes, whether it plays, and
/// whether it is written on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageAbilities {
    /// The actions that rewrite the file in place, each of which the stage does.
    pub edits: &'static [FileAction],
    /// The format the Export sheet writes, or `None` when the stage has no export.
    pub export: Option<ExportFamily>,
    /// Whether the file plays.
    pub playing: Playing,
    /// Whether the file is written on.
    pub writing: Writing,
}

/// A stage that changes nothing and writes nothing.
const READ_ONLY: StageAbilities = StageAbilities {
    edits: &[],
    export: None,
    playing: Playing::Still,
    writing: Writing::Fixed,
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
    playing: Playing::Still,
    writing: Writing::Fixed,
};

/// A PDF turns the page the person is on (the file stores it as the page's `/Rotate`), and does
/// not flip.
const PDF: StageAbilities = StageAbilities {
    edits: &[FileAction::RotateLeft, FileAction::RotateRight],
    export: Some(ExportFamily::Pdf),
    playing: Playing::Still,
    writing: Writing::Writable,
};

/// A recording is trimmed to the part that is kept.
const MEDIA: StageAbilities = StageAbilities {
    edits: &[FileAction::Trim],
    export: Some(ExportFamily::Media),
    playing: Playing::Plays,
    writing: Writing::Fixed,
};

const TEXT: StageAbilities = StageAbilities {
    export: Some(ExportFamily::Text),
    ..READ_ONLY
};

/// A table or a tree is edited in place and written back, which keeps the file's versions to go
/// back to. (A file the viewer cannot write back says so in its edit offer, and offers nothing.)
const DATA: StageAbilities = StageAbilities {
    edits: &[FileAction::RevertTo],
    ..READ_ONLY
};

impl Stage {
    /// What this stage can do. The one match on the stage for the file actions it carries out.
    #[must_use]
    pub fn abilities(&self) -> StageAbilities {
        match self {
            Stage::Raster(_) => RASTER,
            Stage::Pdf(_) => PDF,
            Stage::Media(_) => MEDIA,
            Stage::Text(_) => TEXT,
            Stage::Table(_) | Stage::Tree(_) => DATA,
            Stage::NoStage => READ_ONLY,
        }
    }
}
