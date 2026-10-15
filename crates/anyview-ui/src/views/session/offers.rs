//! What the open file allows and offers: whether it plays, whether an edit is possible, whether
//! the Export sheet has anything to write, and the single match that says which file action
//! applies.

use crate::{
    Allowance, Changes, EditOffer, FileAccess, PlatformAbilities, Playing, StageAbilities,
};
use anyview_core::FileAction;

/// Whether the open recording can be played: without the player (or the plugin that reads it) it
/// shows its facts, and nothing that plays is on offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Playback {
    /// There is a player, or the file is not a recording.
    Playable,
    /// A recording nothing here plays.
    Unplayable,
}

/// Whether the Export sheet has anything to write: a recording with no export on offer and no
/// package to name has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Exports {
    /// The sheet has a format to write.
    Offered,
    /// The sheet would be empty.
    Nothing,
}

/// Whether the open file takes `action`, from what the showing stage declares it can do
/// (`Stage::abilities`), what the file allows and what the platform has. One match, so a file
/// action added to the vocabulary has to say where it applies, and nothing is listed that would do
/// nothing: Copy File waits until the desktop's clipboard can hold a file (the host's clipboard
/// carries text only), what plays waits for a player, what edits waits for a stage that edits and a
/// file that takes a save, and what needs a desktop service waits for the platform to have it.
pub(super) fn offered(action: FileAction, ability: StageAbilities, offers: Offers) -> bool {
    let Offers {
        playback,
        edit,
        access,
        exports,
        platform,
        tool: _,
    } = offers;
    if !platform.offers(action) {
        return false;
    }
    let writable = access == FileAccess::Writable;
    match action {
        FileAction::CopyFile => false,
        FileAction::PlayInMiniWindow | FileAction::PlayInBackground => {
            ability.playing == Playing::Plays && playback == Playback::Playable
        }
        FileAction::RotateLeft
        | FileAction::RotateRight
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical => {
            ability.edits.contains(&action) && edit != EditOffer::Withheld && writable
        }
        FileAction::Trim => {
            ability.edits.contains(&action)
                && edit != EditOffer::Withheld
                && writable
                && playback == Playback::Playable
                && exports == Exports::Offered
        }
        FileAction::RevertTo => {
            !ability.edits.is_empty() && edit != EditOffer::Withheld && writable
        }
        FileAction::Export | FileAction::ConvertTo => {
            ability.export.is_some() && exports == Exports::Offered
        }
        FileAction::Open
        | FileAction::RevealInFolder
        | FileAction::CopyPath
        | FileAction::Share
        | FileAction::Rename
        | FileAction::Duplicate
        | FileAction::MoveToTrash
        | FileAction::Print
        | FileAction::SaveCopy => true,
    }
}

/// What editing the open picture offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::views) enum PictureOffer {
    /// The file is not a picture that can be saved with changes.
    Unavailable,
    /// It can be, and nothing has been done to it.
    Pristine,
    /// It can be, and there are changes that are not saved.
    Edited,
}

/// What marking the open PDF offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::views) struct MarkupOffer {
    /// Whether the document lets the person make marks.
    pub marking: Allowance,
    /// Whether there are fields filled or marks made that are not saved.
    pub changes: Changes,
    /// The mark that is picked, which Delete Mark takes away.
    pub picked: Option<usize>,
}

/// What the open file allows and offers, as the commands' filters read it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Offers {
    /// Whether the file can be played.
    pub playback: Playback,
    /// What a picture edit costs, or that none is possible.
    pub edit: EditOffer,
    /// Whether the file takes a save in place.
    pub access: FileAccess,
    /// Whether the Export sheet has anything to write.
    pub exports: Exports,
    /// The desktop services there are for the file actions that need one.
    pub platform: PlatformAbilities,
    /// The pointer tool in use: the palette lists the other.
    pub tool: crate::Tool,
}
