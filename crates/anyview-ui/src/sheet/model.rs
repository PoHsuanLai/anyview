//! The sheet's states, inputs and outputs.

use super::draft::{ExportDraft, ExportKindPick};
use super::helper::{HelperEnd, HelperPhase};
use super::offer::MediaOffer;
use super::versions::{VersionKey, VersionList};
use crate::edits::{EditCaution, EditOffer, EditRequest};
use crate::typed::TypedText;
use anyview_core::{Fact, Helper};
use ds_core::vocab::ShortcutKey;

/// What opening a sheet reads besides its input: what the host can write.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SheetParams {
    /// The media exports on offer for the open recording.
    pub media: MediaOffer,
    /// What an edit of the open file costs.
    pub edit: EditOffer,
}

/// Which sheet is up, with what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Sheet {
    /// None.
    #[default]
    Closed,
    /// Choosing an export.
    Export { draft: ExportDraft },
    /// An export the viewer cannot offer: the row says which package adds it.
    Unavailable {
        needs: Fact,
        /// The tool to offer to install for it, when the host can.
        helper: Option<Helper>,
    },
    /// Asking before the file goes to the trash.
    ConfirmTrash,
    /// Asking before an edit that loses something is saved.
    ConfirmEdit {
        request: EditRequest,
        caution: EditCaution,
    },
    /// Typing a new name.
    Rename { name: TypedText },
    /// Typing the name of a copy to save beside the file, or a path to save it at.
    SaveCopy { name: TypedText },
    /// Choosing which kept version of the file to go back to, the newest picked to begin with.
    Revert {
        versions: VersionList,
        chosen: VersionKey,
    },
    /// The file has no kept version to go back to.
    NoVersions,
    /// Asking to install a tool the open file needs, and then following the install.
    Helper { helper: Helper, phase: HelperPhase },
}

/// What moves the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetIn {
    /// Open the export sheet on this draft (the default of the file's format).
    OpenExport(ExportDraft),
    /// Open the sheet that says nothing can be exported yet and which package adds it.
    OpenUnavailable(Fact, Option<Helper>),
    /// Ask whether to trash the file.
    AskTrash,
    /// Ask whether to go ahead with an edit that loses this.
    AskEdit(EditRequest, EditCaution),
    /// Ask for a new name, starting from the current one.
    AskRename(TypedText),
    /// Ask for the name of a copy, starting from this one.
    AskSaveCopy(TypedText),
    /// Open the sheet that lists the kept versions of the file; none opens the sheet that says
    /// there are none.
    OpenRevert(Option<VersionList>),
    /// Ask whether to install this tool, which the open file needs.
    OfferHelper(Helper),
    /// Asking the system to install this tool ended like this.
    HelperEnded(Helper, HelperEnd),
    /// The list chose a version.
    PickVersion(VersionKey),
    /// The format pop-up chose a kind.
    PickKind(ExportKindPick),
    /// An option of the export changed; the draft must stay in its format.
    Change(ExportDraft),
    /// The name field now holds this text.
    Typed(TypedText),
    /// The sheet's default button.
    Confirm,
    /// Esc or Cancel.
    Cancel,
    /// The clock; a sheet keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for SheetIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        SheetIn::Elapsed
    }
}

impl SheetIn {
    /// What a key means to an open sheet: Enter confirms and Esc cancels. Every other key means
    /// nothing to the sheet, and it keeps them from the content behind it.
    pub fn from_key(keys: &[ShortcutKey]) -> Option<SheetIn> {
        match keys {
            [ShortcutKey::Enter] => Some(SheetIn::Confirm),
            [ShortcutKey::Escape] => Some(SheetIn::Cancel),
            _ => None,
        }
    }
}

/// What the sheet wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetOut {
    /// Show the sheet.
    Opened,
    /// Hide the sheet.
    Closed,
    /// Write this export.
    Export(ExportDraft),
    /// Move the file to the trash.
    Trash,
    /// Save this edit in place.
    Edit(EditRequest),
    /// Rename the file to this.
    Rename(TypedText),
    /// Write a copy of the file under this name.
    SaveCopy(TypedText),
    /// Put this kept version back as the file.
    Revert(VersionKey),
    /// Install this tool; the answer is `HelperEnded`.
    Provide(Helper),
    /// Open the file again: a tool it needed is installed.
    Reopen,
}
