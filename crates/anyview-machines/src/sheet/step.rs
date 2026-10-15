//! The sheet's transitions: modal, so a sheet that is up ignores every request to open another.

use super::ask::{
    confirm_edit, confirm_replace, confirm_trash, no_versions, rename, revert, save_copy, unsaved,
};
use super::draft::ExportDraft;
use super::helper::HelperPhase;
use super::install::{helping, unavailable};
use super::model::{Sheet, SheetIn, SheetOut, SheetParams};
use super::option::PageSpan;
use super::picture::{PictureSheet, PictureSheetIn, PictureSheetOut};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

pub(super) type Step = (Sheet, Vec<SheetOut>);

impl Machine for Sheet {
    type In = SheetIn;
    type Out = SheetOut;
    type Params = SheetParams;
    type Ctx = ();

    fn step(self, input: SheetIn, _at: Stamp, params: &SheetParams, _cx: &()) -> Step {
        match self {
            Sheet::Closed => closed(input, params),
            Sheet::Export { draft, span } => export(draft, span, input, params),
            Sheet::Unavailable {
                needs,
                helper,
                wanted,
            } => unavailable(needs, helper, wanted, input),
            Sheet::ConfirmTrash => confirm_trash(input),
            Sheet::ConfirmEdit { request, caution } => confirm_edit(request, caution, input),
            Sheet::Rename { name } => rename(name, input),
            Sheet::SaveCopy { name } => save_copy(name, input),
            Sheet::Revert { versions, chosen } => revert(versions, chosen, input),
            Sheet::NoVersions => no_versions(input),
            Sheet::Helper { helper, phase } => helping(helper, phase, input),
            Sheet::Picture(sheet) => picture(sheet, input),
            Sheet::Unsaved(departure) => unsaved(departure, input),
            Sheet::ConfirmReplace => confirm_replace(input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Sheet::Closed
            | Sheet::Export { draft: _, span: _ }
            | Sheet::Unavailable {
                needs: _,
                helper: _,
                wanted: _,
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
            }
            | Sheet::Picture(_)
            | Sheet::Unsaved(_)
            | Sheet::ConfirmReplace => None,
        }
    }
}

fn opened(sheet: Sheet) -> Step {
    (sheet, vec![SheetOut::Opened])
}

pub(super) fn closing(out: SheetOut) -> Step {
    (Sheet::Closed, vec![out, SheetOut::Closed])
}

pub(super) fn cancelled() -> Step {
    (Sheet::Closed, vec![SheetOut::Closed])
}

fn closed(input: SheetIn, params: &SheetParams) -> Step {
    match input {
        SheetIn::OpenExport(draft) => opened(Sheet::Export {
            draft: draft.seeded(&params.export),
            span: PageSpan::All,
        }),
        SheetIn::OpenUnavailable(needs, helper, wanted) => opened(Sheet::Unavailable {
            needs,
            helper,
            wanted,
        }),
        SheetIn::OfferHelper(helper) => opened(Sheet::Helper {
            helper,
            phase: HelperPhase::Ask,
        }),
        SheetIn::AskTrash => opened(Sheet::ConfirmTrash),
        SheetIn::AskPicture(sheet) => opened(Sheet::Picture(sheet)),
        SheetIn::AskUnsaved(departure) => opened(Sheet::Unsaved(departure)),
        SheetIn::AskReplace => opened(Sheet::ConfirmReplace),
        SheetIn::AskEdit(request, caution) => opened(Sheet::ConfirmEdit { request, caution }),
        SheetIn::AskRename(name) => opened(Sheet::Rename { name }),
        SheetIn::AskSaveCopy(name) => opened(Sheet::SaveCopy { name }),
        SheetIn::OpenRevert(Some(versions)) => match versions.newest().cloned() {
            Some(chosen) => opened(Sheet::Revert { versions, chosen }),
            None => opened(Sheet::NoVersions),
        },
        SheetIn::OpenRevert(None) => opened(Sheet::NoVersions),
        SheetIn::HelperEnded(_, _)
        | SheetIn::Picture(_)
        | SheetIn::PickVersion(_)
        | SheetIn::PickKind(_)
        | SheetIn::Tune(_)
        | SheetIn::Change(_)
        | SheetIn::Typed(_)
        | SheetIn::Discard
        | SheetIn::Confirm
        | SheetIn::Cancel
        | SheetIn::Elapsed => (Sheet::Closed, vec![]),
    }
}

fn export(draft: ExportDraft, span: PageSpan, input: SheetIn, params: &SheetParams) -> Step {
    let keep = |draft, span| (Sheet::Export { draft, span }, vec![]);
    match input {
        SheetIn::PickKind(pick) => match draft.picked(pick) {
            Some(picked) => keep(picked.seeded(&params.export), PageSpan::All),
            None => keep(draft, span),
        },
        SheetIn::Change(changed) if changed.family() == draft.family() => keep(changed, span),
        SheetIn::Tune(option) => {
            let (tuned, span) = draft.tuned(option, &params.export, span);
            keep(tuned, span)
        }
        SheetIn::Confirm => closing(SheetOut::Export(draft)),
        SheetIn::Cancel => cancelled(),
        SheetIn::Change(_)
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
        | SheetIn::Typed(_)
        | SheetIn::Elapsed => keep(draft, span),
    }
}

/// A sheet of editing a picture. Adjust Size keeps the sizes being chosen and Return takes them;
/// Esc puts it away.
fn picture(sheet: PictureSheet, input: SheetIn) -> Step {
    match (sheet, input) {
        (PictureSheet::Resize(draft), SheetIn::Confirm) => {
            closing(SheetOut::Picture(PictureSheetOut::Resize(draft.size())))
        }
        (PictureSheet::Resize(draft), SheetIn::Picture(PictureSheetIn::Resize(change))) => (
            Sheet::Picture(PictureSheet::Resize(draft.changed(change))),
            vec![],
        ),
        (_, SheetIn::Cancel) => cancelled(),
        (
            sheet,
            SheetIn::OpenExport(_)
            | SheetIn::OpenUnavailable(_, _, _)
            | SheetIn::OfferHelper(_)
            | SheetIn::HelperEnded(_, _)
            | SheetIn::AskTrash
            | SheetIn::AskPicture(_)
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
            | SheetIn::Elapsed,
        ) => (Sheet::Picture(sheet), vec![]),
    }
}
