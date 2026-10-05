//! The sheet's transitions: modal, so a sheet that is up ignores every request to open another.

use super::draft::ExportDraft;
use super::model::{Sheet, SheetIn, SheetOut};
use super::versions::{VersionKey, VersionList};
use crate::typed::TypedText;
use anyview_core::Fact;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Sheet, Vec<SheetOut>);

impl Machine for Sheet {
    type In = SheetIn;
    type Out = SheetOut;
    type Params = ();
    type Ctx = ();

    fn step(self, input: SheetIn, _at: Stamp, _params: &(), _cx: &()) -> Step {
        match self {
            Sheet::Closed => closed(input),
            Sheet::Export { draft } => export(draft, input),
            Sheet::Unavailable { needs } => unavailable(needs, input),
            Sheet::ConfirmTrash => confirm_trash(input),
            Sheet::Rename { name } => rename(name, input),
            Sheet::SaveCopy { name } => save_copy(name, input),
            Sheet::Revert { versions, chosen } => revert(versions, chosen, input),
            Sheet::NoVersions => no_versions(input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Sheet::Closed
            | Sheet::Export { draft: _ }
            | Sheet::Unavailable { needs: _ }
            | Sheet::ConfirmTrash
            | Sheet::Rename { name: _ }
            | Sheet::SaveCopy { name: _ }
            | Sheet::Revert {
                versions: _,
                chosen: _,
            }
            | Sheet::NoVersions => None,
        }
    }
}

fn opened(sheet: Sheet) -> Step {
    (sheet, vec![SheetOut::Opened])
}

fn closing(out: SheetOut) -> Step {
    (Sheet::Closed, vec![out, SheetOut::Closed])
}

fn cancelled() -> Step {
    (Sheet::Closed, vec![SheetOut::Closed])
}

fn closed(input: SheetIn) -> Step {
    match input {
        SheetIn::OpenExport(draft) => opened(Sheet::Export { draft }),
        SheetIn::OpenUnavailable(needs) => opened(Sheet::Unavailable { needs }),
        SheetIn::AskTrash => opened(Sheet::ConfirmTrash),
        SheetIn::AskRename(name) => opened(Sheet::Rename { name }),
        SheetIn::AskSaveCopy(name) => opened(Sheet::SaveCopy { name }),
        SheetIn::OpenRevert(Some(versions)) => match versions.newest().cloned() {
            Some(chosen) => opened(Sheet::Revert { versions, chosen }),
            None => opened(Sheet::NoVersions),
        },
        SheetIn::OpenRevert(None) => opened(Sheet::NoVersions),
        SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Confirm
        | SheetIn::Cancel
        | SheetIn::Elapsed => (Sheet::Closed, vec![]),
    }
}

fn export(draft: ExportDraft, input: SheetIn) -> Step {
    let keep = |draft| (Sheet::Export { draft }, vec![]);
    match input {
        SheetIn::PickKind(pick) => keep(draft.picked(pick).unwrap_or(draft)),
        SheetIn::Change(changed) if changed.family() == draft.family() => keep(changed),
        SheetIn::Confirm => closing(SheetOut::Export(draft)),
        SheetIn::Cancel => cancelled(),
        SheetIn::Change(_)
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => keep(draft),
    }
}

/// The sheet that says what is missing: Enter and Esc both put it away, and nothing is written.
fn unavailable(needs: Fact, input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm | SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::Unavailable { needs }, vec![]),
    }
}

fn confirm_trash(input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Trash),
        SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::ConfirmTrash, vec![]),
    }
}

fn rename(name: TypedText, input: SheetIn) -> Step {
    match input {
        SheetIn::Typed(text) => (Sheet::Rename { name: text }, vec![]),
        SheetIn::Confirm if !name.is_empty() => closing(SheetOut::Rename(name)),
        SheetIn::Cancel => cancelled(),
        SheetIn::Confirm
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Elapsed => (Sheet::Rename { name }, vec![]),
    }
}

fn save_copy(name: TypedText, input: SheetIn) -> Step {
    match input {
        SheetIn::Typed(text) => (Sheet::SaveCopy { name: text }, vec![]),
        SheetIn::Confirm if !name.is_empty() => closing(SheetOut::SaveCopy(name)),
        SheetIn::Cancel => cancelled(),
        SheetIn::Confirm
        | SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Elapsed => (Sheet::SaveCopy { name }, vec![]),
    }
}

/// Choosing a version: only a row of the list can be picked, and Enter goes back to the one
/// chosen.
fn revert(versions: VersionList, chosen: VersionKey, input: SheetIn) -> Step {
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
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::Revert { versions, chosen }, vec![]),
    }
}

/// The sheet that says there is nothing to go back to: Enter and Esc both put it away.
fn no_versions(input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm | SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_)
        | SheetIn::AskTrash
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::NoVersions, vec![]),
    }
}
