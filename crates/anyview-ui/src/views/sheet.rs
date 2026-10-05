//! The modal sheets: export, rename, save a copy, revert to a version and the trash question. The sheet machine says which is open
//! and what it holds; these draw them and report the person's choices as sheet inputs.

use crate::EditCaution;
use crate::{ExportDraft, ExportKindPick, MediaOffer, TypedText, VersionKey, VersionList};
use anyview_core::{Edit, Fact};
use dioxus::prelude::*;
use ds::components::controls::button_model::Answers;
use ds::components::controls::segmented::Tracking;
use ds::components::overlays::alert_model::{AlertButton, AlertRole};
use ds::prelude::{
    Alert, Button, Choice, FieldFocus, RadioGroup, SegmentedControl, Sheet, TextField,
};
use ds_core::word::Word;

/// "Move to Trash?": Esc cancels, the destructive button is never the default.
#[component]
pub(super) fn TrashSheet(
    name: String,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    rsx! {
        Alert {
            title: "Move to Trash?",
            message: Some(format!("“{name}” will be moved to the Trash.").into()),
            buttons: vec![
                AlertButton::new("Cancel", AlertRole::Cancel, oncancel),
                AlertButton::new("Move to Trash", AlertRole::Destructive, onconfirm),
            ],
        }
    }
}

/// Asking before an edit that loses something is saved: one short sentence of what is lost, the
/// edit's own button as the default and Cancel.
#[component]
pub(super) fn EditSheet(
    edit: Edit,
    caution: EditCaution,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    let (title, verb) = match (edit, caution) {
        (_, EditCaution::Signed) => ("Change this document?", "Change Pages"),
        (Edit::Flip(_), EditCaution::Loses(_)) => ("Flip this picture?", "Flip"),
        (_, EditCaution::Loses(_)) => ("Rotate this picture?", "Rotate"),
    };
    let message = match caution {
        EditCaution::Signed => {
            "This document is signed. Changing its pages will remove the signature.".to_owned()
        }
        EditCaution::Loses(lost) => format!(
            "Saving it this way may lose some details from the original. {lost} You can go back to the original with Revert To."
        ),
    };
    rsx! {
        Alert {
            title,
            message: Some(message.into()),
            buttons: vec![
                AlertButton::new(verb, AlertRole::Normal, onconfirm),
                AlertButton::new("Cancel", AlertRole::Cancel, oncancel),
            ],
        }
    }
}

/// Typing a name: a new one for the file, or the one a copy is saved under. `label` names the
/// sheet and its field's purpose, `confirm` its default button.
#[component]
pub(super) fn NameSheet(
    label: &'static str,
    confirm: &'static str,
    name: TypedText,
    ontyped: EventHandler<TypedText>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    rsx! {
        Sheet { label, onclose: move |()| oncancel.call(()),
            div { class: "viewer-sheet",
                TextField {
                    label: "Name",
                    value: name.as_str().to_owned(),
                    focus: FieldFocus::OnMount,
                    oninput: move |text: String| ontyped.call(TypedText::new(text)),
                }
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", onclick: move |_| oncancel.call(()) }
                    Button { label: confirm, answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                }
            }
        }
    }
}

/// Choosing which kept version of the file to go back to, newest first.
#[component]
pub(super) fn RevertSheet(
    versions: VersionList,
    chosen: VersionKey,
    onpick: EventHandler<VersionKey>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    let choices: Vec<Choice<VersionKey>> = versions
        .rows()
        .iter()
        .map(|row| Choice::new(row.key.clone(), row.label()))
        .collect();
    rsx! {
        Sheet { label: "Revert To", onclose: move |()| oncancel.call(()),
            div { class: "viewer-sheet",
                RadioGroup::<VersionKey> {
                    label: "Version",
                    choices,
                    value: chosen,
                    onchange: move |key: VersionKey| onpick.call(key),
                }
                p { class: "viewer-sheet-note",
                    "The file as it is now is kept too, so going back can be undone."
                }
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", onclick: move |_| oncancel.call(()) }
                    Button { label: "Revert", answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                }
            }
        }
    }
}

/// The file has no earlier version: Enter and Esc put this away.
#[component]
pub(super) fn NoVersionsSheet(onclose: EventHandler<()>) -> Element {
    rsx! {
        Sheet { label: "Revert To", onclose: move |()| onclose.call(()),
            div { class: "viewer-sheet",
                p { class: "viewer-sheet-note", "This file has no earlier version." }
                div { class: "viewer-sheet-buttons",
                    Button { label: "OK", answers: Answers::Return, onclick: move |_| onclose.call(()) }
                }
            }
        }
    }
}

/// Choosing what to export: the format, then Export or Cancel. Only the media formats `offer`
/// holds are listed; when more would be there with another package, the sheet says which.
#[component]
pub(super) fn ExportSheet(
    draft: ExportDraft,
    offer: MediaOffer,
    onpick: EventHandler<ExportKindPick>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    let choices: Vec<Choice<ExportKindPick>> = draft
        .choices_within(&offer)
        .into_iter()
        .map(|(pick, label)| Choice::new(pick, label))
        .collect();
    rsx! {
        Sheet { label: "Export", onclose: move |()| oncancel.call(()),
            div { class: "viewer-sheet",
                SegmentedControl::<ExportKindPick> {
                    label: "Format",
                    choices,
                    tracking: Tracking::SelectOne(draft.pick()),
                    onchange: move |pick: ExportKindPick| onpick.call(pick),
                }
                if let Some(needs) = offer.needs() {
                    p { class: "viewer-sheet-note",
                        "{needs.label.label()}: {needs.value.as_str()}"
                    }
                }
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", onclick: move |_| oncancel.call(()) }
                    Button { label: "Export", answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                }
            }
        }
    }
}

/// An export the viewer cannot offer at all: which package adds it. Enter and Esc put it away.
#[component]
pub(super) fn UnavailableSheet(needs: Fact, onclose: EventHandler<()>) -> Element {
    rsx! {
        Sheet { label: "Export", onclose: move |()| onclose.call(()),
            div { class: "viewer-sheet",
                p { class: "viewer-sheet-note", "There is nothing to export yet." }
                p { class: "viewer-sheet-note",
                    "{needs.label.label()}: {needs.value.as_str()}"
                }
                div { class: "viewer-sheet-buttons",
                    Button { label: "OK", answers: Answers::Return, onclick: move |_| onclose.call(()) }
                }
            }
        }
    }
}
