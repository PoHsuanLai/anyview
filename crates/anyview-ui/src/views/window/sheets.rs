//! The sheets the viewer window can have up, each drawn from the machine's `Sheet` and answering
//! with a sheet input.

use super::title_of;
use crate::views::export::ExportSheet;
use crate::views::resize::ResizeSheet;
use crate::views::sheet::{
    EditSheet, InstallSheet, NameSheet, NoVersionsSheet, ReplaceSheet, RevertSheet, TrashSheet,
    UnavailableSheet,
};
use crate::views::shelf::{Dispatch, Shelf};
use crate::views::unsaved::UnsavedSheet;
use crate::{
    ExportFacts, MediaOffer, PictureSheet, PictureSheetIn, Sheet, SheetIn, TypedText, ViewerIn,
};
use dioxus::prelude::*;

/// What the open `sheet` shows, or nothing for a closed one. `export_facts` and `offer` are what
/// the Export sheet is told of the file, and `edge` names the helper an install sheet offers.
pub(super) fn sheet_view(
    sheet: &Sheet,
    dispatch: Dispatch,
    shelf: Shelf,
    edge: &crate::Edge,
    export_facts: ExportFacts,
    offer: &MediaOffer,
) -> Element {
    match sheet {
        Sheet::Closed => rsx! {},
        Sheet::ConfirmTrash => rsx! {
            TrashSheet {
                name: title_of(&shelf.probe.read()).unwrap_or_default(),
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::ConfirmEdit { request, caution } => rsx! {
            EditSheet {
                edit: request.edit,
                caution: *caution,
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::ConfirmReplace => rsx! {
            ReplaceSheet {
                name: title_of(&shelf.probe.read()).unwrap_or_default(),
                onreplace: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::Rename { name } => rsx! {
            NameSheet {
                label: "Rename",
                confirm: "Rename",
                name: name.clone(),
                ontyped: move |text: TypedText| dispatch.send(ViewerIn::Sheet(SheetIn::Typed(text))),
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::SaveCopy { name } => rsx! {
            NameSheet {
                label: "Save a Copy",
                confirm: "Save",
                name: name.clone(),
                ontyped: move |text: TypedText| dispatch.send(ViewerIn::Sheet(SheetIn::Typed(text))),
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::Revert { versions, chosen } => rsx! {
            RevertSheet {
                versions: versions.clone(),
                chosen: chosen.clone(),
                onpick: move |key| dispatch.send(ViewerIn::Sheet(SheetIn::PickVersion(key))),
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::NoVersions => rsx! {
            NoVersionsSheet {
                onclose: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::Unavailable {
            needs,
            helper,
            wanted,
        } => rsx! {
            UnavailableSheet {
                needs: needs.clone(),
                helper: *helper,
                wanted: *wanted,
                onclose: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                oninstall: move |helper| dispatch.send(ViewerIn::Sheet(SheetIn::OfferHelper(helper))),
            }
        },
        Sheet::Helper { helper, phase } => match edge.helper_words(*helper) {
            Some(words) => rsx! {
                InstallSheet {
                    words,
                    phase: phase.clone(),
                    oninstall: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                    ondismiss: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                }
            },
            None => rsx! {},
        },
        Sheet::Picture(PictureSheet::Resize(draft)) => rsx! {
            ResizeSheet {
                draft: *draft,
                onchange: move |change| dispatch.send(ViewerIn::Sheet(SheetIn::Picture(PictureSheetIn::Resize(change)))),
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::Unsaved(_) => rsx! {
            UnsavedSheet {
                name: title_of(&shelf.probe.read()).unwrap_or_default(),
                onsave: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                ondiscard: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Discard)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
        Sheet::Export { draft, span } => rsx! {
            ExportSheet {
                draft: *draft,
                span: *span,
                facts: export_facts,
                offer: offer.clone(),
                name: title_of(&shelf.probe.read()).unwrap_or_default(),
                onpick: move |pick| dispatch.send(ViewerIn::Sheet(SheetIn::PickKind(pick))),
                ontune: move |option| dispatch.send(ViewerIn::Sheet(SheetIn::Tune(option))),
                onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
            }
        },
    }
}
