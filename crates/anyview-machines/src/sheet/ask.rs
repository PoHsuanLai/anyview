//! The sheets that put a question and take its answer: trash, rename, save a copy, revert, a kept edit, unsaved changes and a replaced file.

use super::departure::Departure;
use super::model::{Sheet, SheetIn, SheetOut};
use super::step::{Step, cancelled, closing};
use super::versions::{VersionKey, VersionList};
use crate::edits::{EditCaution, EditRequest};
use crate::typed::TypedText;

pub(super) fn confirm_trash(input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Trash),
        SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::Picture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Discard
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::ConfirmTrash, vec![]),
    }
}

pub(super) fn rename(name: TypedText, input: SheetIn) -> Step {
    match input {
        SheetIn::Typed(text) => (Sheet::Rename { name: text }, vec![]),
        SheetIn::Confirm if !name.is_empty() => closing(SheetOut::Rename(name)),
        SheetIn::Cancel => cancelled(),
        SheetIn::Confirm
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::Picture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Discard
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Elapsed => (Sheet::Rename { name }, vec![]),
    }
}

pub(super) fn save_copy(name: TypedText, input: SheetIn) -> Step {
    match input {
        SheetIn::Typed(text) => (Sheet::SaveCopy { name: text }, vec![]),
        SheetIn::Confirm if !name.is_empty() => closing(SheetOut::SaveCopy(name)),
        SheetIn::Cancel => cancelled(),
        SheetIn::Confirm
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::Picture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Discard
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Elapsed => (Sheet::SaveCopy { name }, vec![]),
    }
}

/// Choosing a version: only a row of the list can be picked, and Enter goes back to the one
/// chosen.
pub(super) fn revert(versions: VersionList, chosen: VersionKey, input: SheetIn) -> Step {
    match input {
        SheetIn::PickVersion(key) if versions.holds(&key) => (
            Sheet::Revert {
                versions,
                chosen: key,
            },
            vec![],
        ),
        SheetIn::Confirm => closing(SheetOut::Revert(chosen)),
        SheetIn::Cancel => cancelled(),
        SheetIn::PickVersion(_)
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::Picture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Discard
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::Revert { versions, chosen }, vec![]),
    }
}

/// The sheet that says there is nothing to go back to: Enter and Esc both put it away.
pub(super) fn no_versions(input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm | SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::Picture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Discard
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::NoVersions, vec![]),
    }
}

pub(super) fn confirm_edit(request: EditRequest, caution: EditCaution, input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Edit(request)),
        SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::Picture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Discard
        | SheetIn::AskEdit(..)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::ConfirmEdit { request, caution }, vec![]),
    }
}

/// The question asked before going on with changes that are not saved: Return saves them first,
/// Don't Save lets them go, Esc keeps the person where they were. What is saved, and where the
/// person goes after, are the viewer's.
pub(super) fn unsaved(departure: Departure, input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Save(departure)),
        SheetIn::Discard => closing(SheetOut::Discard(departure)),
        SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Picture(_)
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Tune(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::Unsaved(departure), vec![]),
    }
}

/// Another program changed the file under the edited text: Return replaces what it wrote with
/// the text, Esc keeps the person where they were.
pub(super) fn confirm_replace(input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Replace),
        SheetIn::Cancel => cancelled(),
        SheetIn::Discard
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskPicture(_)
        | SheetIn::AskUnsaved(_)
        | SheetIn::AskReplace
        | SheetIn::Picture(_)
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Tune(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::ConfirmReplace, vec![]),
    }
}
