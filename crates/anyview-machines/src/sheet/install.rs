//! The sheets about a missing tool: what is missing, and the install that fixes it.

use super::helper::{HelperEnd, HelperPhase};
use super::model::{Sheet, SheetIn, SheetOut, Wanted};
use super::step::{Step, cancelled};
use anyview_core::{Fact, Helper};

/// The sheet that says what is missing: Enter and Esc both put it away, and nothing is written.
/// When the host can install the tool that is missing, the sheet's own button turns it into the
/// install question, which takes the same place.
pub(super) fn unavailable(
    needs: Fact,
    helper: Option<Helper>,
    wanted: Wanted,
    input: SheetIn,
) -> Step {
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
        | SheetIn::Elapsed => (
            Sheet::Unavailable {
                needs,
                helper,
                wanted,
            },
            vec![],
        ),
    }
}

/// The install sheet. Return installs and Esc says "Not Now" while it asks; once the install has
/// ended badly either closes it; while the system installs, neither does anything, since the
/// password prompt is the system's and an answer is coming. What ends an install for another
/// tool is not this sheet's business.
pub(super) fn helping(helper: Helper, phase: HelperPhase, input: SheetIn) -> Step {
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
            | HelperPhase::Unsupported { .. },
            SheetIn::Cancel,
        )
        | (
            HelperPhase::Failed(_) | HelperPhase::NotFound(_) | HelperPhase::Unsupported { .. },
            SheetIn::Confirm,
        ) => cancelled(),
        (_, SheetIn::HelperEnded(ended, end)) if ended == helper => ended_with(helper, end),
        (
            HelperPhase::Ask
            | HelperPhase::Installing
            | HelperPhase::Failed(_)
            | HelperPhase::NotFound(_)
            | HelperPhase::Unsupported { .. },
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
        HelperEnd::Unsupported { program, command } => {
            phase(HelperPhase::Unsupported { program, command })
        }
        HelperEnd::Failed(reason) => phase(HelperPhase::Failed(reason)),
    }
}
