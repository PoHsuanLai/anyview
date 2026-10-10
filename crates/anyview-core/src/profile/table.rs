//! What each kind of file offers: its actions, its edits and whether the viewer has a stage for it.
//! The one match on `FormatKind`; `actions_for`, `edits_for` and `stage_support` read from it.

use crate::action::FileAction;
use crate::edit::EditKind;
use crate::kind::{FormatKind, Mime};
use crate::peek::StageSupport;

/// Everything that varies by kind of file.
#[derive(Debug, Clone, Copy)]
struct KindProfile {
    mime: &'static str,
    actions: &'static [FileAction],
    edits: &'static [EditKind],
    stage: StageSupport,
}

use FileAction::{
    ConvertTo, CopyFile, CopyPath, Duplicate, Export, FlipHorizontal, FlipVertical, MoveToTrash,
    Open, PlayInBackground, PlayInMiniWindow, Print, Rename, RevealInFolder, RevertTo, RotateLeft,
    RotateRight, SaveCopy, Share,
};

/// A file with nothing to do but handle it.
const PLAIN: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
];
const IMAGE: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
    Print,
    Export,
    ConvertTo,
    SaveCopy,
    RevertTo,
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
];
const PDF: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
    Print,
    Export,
    ConvertTo,
    SaveCopy,
    RevertTo,
    RotateLeft,
    RotateRight,
];
const VECTOR: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
    Print,
    Export,
];
const VIDEO: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
    Export,
    PlayInMiniWindow,
];
const AUDIO: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
    Export,
    ConvertTo,
    PlayInBackground,
];
const TEXT: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
    Print,
    Export,
    ConvertTo,
];
const FOLDER: &[FileAction] = &[
    Open,
    RevealInFolder,
    CopyFile,
    CopyPath,
    Share,
    Rename,
    Duplicate,
    MoveToTrash,
];

const NO_EDITS: &[EditKind] = &[];
const IMAGE_EDITS: &[EditKind] = &[EditKind::Rotate, EditKind::Flip, EditKind::Adjust];
const PDF_EDITS: &[EditKind] = &[EditKind::Rotate, EditKind::DeletePages, EditKind::MovePage];

const fn profile(
    mime: &'static str,
    actions: &'static [FileAction],
    edits: &'static [EditKind],
    stage: StageSupport,
) -> KindProfile {
    KindProfile {
        mime,
        actions,
        edits,
        stage,
    }
}

fn profile_of(kind: FormatKind) -> KindProfile {
    use StageSupport::{PeekOnly, Stage};
    match kind {
        FormatKind::Pdf => profile("application/pdf", PDF, PDF_EDITS, Stage),
        FormatKind::Raster => profile("image/png", IMAGE, IMAGE_EDITS, Stage),
        FormatKind::Vector => profile("image/svg+xml", VECTOR, NO_EDITS, Stage),
        FormatKind::Video => profile("video/mp4", VIDEO, NO_EDITS, Stage),
        FormatKind::Audio => profile("audio/mpeg", AUDIO, NO_EDITS, Stage),
        FormatKind::Markdown => profile("text/markdown", TEXT, NO_EDITS, Stage),
        FormatKind::Code | FormatKind::PlainText => profile("text/plain", TEXT, NO_EDITS, Stage),
        FormatKind::Table => profile("text/csv", PLAIN, NO_EDITS, Stage),
        FormatKind::Tree => profile("application/json", PLAIN, NO_EDITS, Stage),
        FormatKind::Font => profile("font/ttf", PLAIN, NO_EDITS, PeekOnly),
        FormatKind::Archive => profile("application/zip", PLAIN, NO_EDITS, PeekOnly),
        FormatKind::Book => profile("application/epub+zip", PLAIN, NO_EDITS, Stage),
        FormatKind::Office => profile(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            PLAIN,
            NO_EDITS,
            PeekOnly,
        ),
        FormatKind::Folder => profile("inode/directory", FOLDER, NO_EDITS, PeekOnly),
        FormatKind::Other => profile("application/octet-stream", PLAIN, NO_EDITS, PeekOnly),
    }
}

/// The actions a file of `kind` offers, in menu order. The launcher shows those whose `reach`
/// includes it, and the viewer those that include the viewer.
pub fn actions_for(kind: FormatKind) -> &'static [FileAction] {
    profile_of(kind).actions
}

/// A media type a file of `kind` has, for a row that knows only the kind (a recently viewed
/// file the history names): the commonest type of the kind. `kind_of_mime` of it is `kind`, except
/// that code and plain text share `text/plain`, which names plain text.
#[must_use]
pub fn mime_for(kind: FormatKind) -> Mime {
    Mime::known(profile_of(kind).mime)
}

/// The sorts of edit a file of `kind` can be saved with in place.
pub fn edits_for(kind: FormatKind) -> &'static [EditKind] {
    profile_of(kind).edits
}

/// Whether the viewer has a full stage for `kind` or shows only the light tier. Every kind is
/// answered: a kind without a stage says `PeekOnly` and offers facts.
#[must_use]
pub fn stage_support(kind: FormatKind) -> StageSupport {
    profile_of(kind).stage
}
