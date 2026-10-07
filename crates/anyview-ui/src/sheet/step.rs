//! The sheet's transitions: modal, so a sheet that is up ignores every request to open another.

use super::draft::ExportDraft;
use super::helper::{HelperEnd, HelperPhase};
use super::model::{Sheet, SheetIn, SheetOut};
use super::versions::{VersionKey, VersionList};
use crate::edits::{EditCaution, EditRequest};
use crate::typed::TypedText;
use anyview_core::{Fact, Helper};
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
            Sheet::Unavailable { needs, helper } => unavailable(needs, helper, input),
            Sheet::ConfirmTrash => confirm_trash(input),
            Sheet::ConfirmEdit { request, caution } => confirm_edit(request, caution, input),
            Sheet::Rename { name } => rename(name, input),
            Sheet::SaveCopy { name } => save_copy(name, input),
            Sheet::Revert { versions, chosen } => revert(versions, chosen, input),
            Sheet::NoVersions => no_versions(input),
            Sheet::Helper { helper, phase } => helping(helper, phase, input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Sheet::Closed
            | Sheet::Export { draft: _ }
            | Sheet::Unavailable {
                needs: _,
                helper: _,
            }
            | Sheet::ConfirmTrash
            | Sheet::ConfirmEdit {
                request: _,
                caution: _,
            }
            | Sheet::Rename { name: _ }
            | Sheet::SaveCopy { name: _ }
            | Sheet::Revert {
                versions: _,
                chosen: _,
            }
            | Sheet::NoVersions
            | Sheet::Helper {
                helper: _,
                phase: _,
            } => None,
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
        SheetIn::OpenUnavailable(needs, helper) => opened(Sheet::Unavailable { needs, helper }),
        SheetIn::OfferHelper(helper) => opened(Sheet::Helper {
            helper,
            phase: HelperPhase::Ask,
        }),
        SheetIn::AskTrash => opened(Sheet::ConfirmTrash),
        SheetIn::AskEdit(request, caution) => opened(Sheet::ConfirmEdit { request, caution }),
        SheetIn::AskRename(name) => opened(Sheet::Rename { name }),
        SheetIn::AskSaveCopy(name) => opened(Sheet::SaveCopy { name }),
        SheetIn::OpenRevert(Some(versions)) => match versions.newest().cloned() {
            Some(chosen) => opened(Sheet::Revert { versions, chosen }),
            None => opened(Sheet::NoVersions),
        },
        SheetIn::OpenRevert(None) => opened(Sheet::NoVersions),
        SheetIn::HelperEnded(_, _)
        | SheetIn::PickVersion(_)
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
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => keep(draft),
    }
}

/// The sheet that says what is missing: Enter and Esc both put it away, and nothing is written.
/// When the host can install the tool that is missing, the sheet's own button turns it into the
/// install question, which takes the same place.
fn unavailable(needs: Fact, helper: Option<Helper>, input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm | SheetIn::Cancel => cancelled(),
        SheetIn::OfferHelper(offered) if helper == Some(offered) => (
            Sheet::Helper {
                helper: offered,
                phase: HelperPhase::Ask,
            },
            vec![],
        ),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::Unavailable { needs, helper }, vec![]),
    }
}

fn confirm_trash(input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Trash),
        SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
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
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
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
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
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
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
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
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(_, _)
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

fn confirm_edit(request: EditRequest, caution: EditCaution, input: SheetIn) -> Step {
    match input {
        SheetIn::Confirm => closing(SheetOut::Edit(request)),
        SheetIn::Cancel => cancelled(),
        SheetIn::OpenExport(_)
        | SheetIn::OpenUnavailable(_, _)
        | SheetIn::OfferHelper(_)
        | SheetIn::HelperEnded(_, _)
        | SheetIn::AskTrash
        | SheetIn::AskEdit(..)
        | SheetIn::AskRename(_)
        | SheetIn::AskSaveCopy(_)
        | SheetIn::OpenRevert(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => (Sheet::ConfirmEdit { request, caution }, vec![]),
    }
}

/// The install sheet. Return installs and Esc says "Not Now" while it asks; once the install has
/// ended badly either closes it; while the system installs, neither does anything, since the
/// password prompt is the system's and an answer is coming. What ends an install for another
/// tool is not this sheet's business.
fn helping(helper: Helper, phase: HelperPhase, input: SheetIn) -> Step {
    let keep = |phase| (Sheet::Helper { helper, phase }, vec![]);
    match (&phase, input) {
        (HelperPhase::Ask, SheetIn::Confirm) => (
            Sheet::Helper {
                helper,
                phase: HelperPhase::Installing,
            },
            vec![SheetOut::Provide(helper)],
        ),
        (
            HelperPhase::Ask
            | HelperPhase::Failed(_)
            | HelperPhase::NotFound(_)
            | HelperPhase::Unsupported(_),
            SheetIn::Cancel,
        )
        | (
            HelperPhase::Failed(_) | HelperPhase::NotFound(_) | HelperPhase::Unsupported(_),
            SheetIn::Confirm,
        ) => cancelled(),
        (_, SheetIn::HelperEnded(ended, end)) if ended == helper => ended_with(helper, end),
        (
            HelperPhase::Ask
            | HelperPhase::Installing
            | HelperPhase::Failed(_)
            | HelperPhase::NotFound(_)
            | HelperPhase::Unsupported(_),
            SheetIn::OpenExport(_)
            | SheetIn::OpenUnavailable(_, _)
            | SheetIn::OfferHelper(_)
            | SheetIn::HelperEnded(_, _)
            | SheetIn::AskTrash
            | SheetIn::AskEdit(_, _)
            | SheetIn::AskRename(_)
            | SheetIn::AskSaveCopy(_)
            | SheetIn::OpenRevert(_)
            | SheetIn::PickVersion(_)
            | SheetIn::PickKind(_)
            | SheetIn::Change(_)
            | SheetIn::Typed(_)
            | SheetIn::Confirm
            | SheetIn::Cancel
            | SheetIn::Elapsed,
        ) => keep(phase),
    }
}

/// An install ended: a tool that arrived closes the sheet and opens the file again, a refusal
/// closes it quietly, and the rest say what happened in the sheet's own phases.
fn ended_with(helper: Helper, end: HelperEnd) -> Step {
    let phase = |phase| (Sheet::Helper { helper, phase }, vec![]);
    match end {
        HelperEnd::Installed => (Sheet::Closed, vec![SheetOut::Reopen, SheetOut::Closed]),
        HelperEnd::Declined => cancelled(),
        HelperEnd::NotFound(package) => phase(HelperPhase::NotFound(package)),
        HelperEnd::Unsupported(program) => phase(HelperPhase::Unsupported(program)),
        HelperEnd::Failed(reason) => phase(HelperPhase::Failed(reason)),
    }
}
