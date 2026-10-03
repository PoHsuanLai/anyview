//! The sheet's states, inputs and outputs.

use super::draft::{ExportDraft, ExportKindPick};
use super::offer::MediaOffer;
use crate::typed::TypedText;
use anyview_core::Fact;
use ds_core::vocab::ShortcutKey;

/// What opening a sheet reads besides its input: what the host can write.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SheetParams {
    /// The media exports on offer for the open recording.
    pub media: MediaOffer,
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
    Unavailable { needs: Fact },
    /// Asking before the file goes to the trash.
    ConfirmTrash,
    /// Typing a new name.
    Rename { name: TypedText },
}

/// What moves the sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetIn {
    /// Open the export sheet on this draft (the default of the file's format).
    OpenExport(ExportDraft),
    /// Open the sheet that says nothing can be exported yet and which package adds it.
    OpenUnavailable(Fact),
    /// Ask whether to trash the file.
    AskTrash,
    /// Ask for a new name, starting from the current one.
    AskRename(TypedText),
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
    /// Rename the file to this.
    Rename(TypedText),
}
