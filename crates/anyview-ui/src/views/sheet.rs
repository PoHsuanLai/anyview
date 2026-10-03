//! The modal sheets: export, rename and the trash question. The sheet machine says which is open
//! and what it holds; these draw them and report the person's choices as sheet inputs.

use crate::{ExportDraft, ExportKindPick, TypedText};
use dioxus::prelude::*;
use ds::components::controls::button_model::Answers;
use ds::components::controls::segmented::Tracking;
use ds::components::overlays::alert_model::{AlertButton, AlertRole};
use ds::prelude::{Alert, Button, Choice, FieldFocus, SegmentedControl, Sheet, TextField};

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

/// Typing a new name.
#[component]
pub(super) fn RenameSheet(
    name: TypedText,
    ontyped: EventHandler<TypedText>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    rsx! {
        Sheet { label: "Rename", onclose: move |()| oncancel.call(()),
            div { class: "viewer-sheet",
                TextField {
                    label: "Name",
                    value: name.as_str().to_owned(),
                    focus: FieldFocus::OnMount,
                    oninput: move |text: String| ontyped.call(TypedText::new(text)),
                }
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", onclick: move |_| oncancel.call(()) }
                    Button { label: "Rename", answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                }
            }
        }
    }
}

/// Choosing what to export: the format, then Export or Cancel.
#[component]
pub(super) fn ExportSheet(
    draft: ExportDraft,
    onpick: EventHandler<ExportKindPick>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    let choices: Vec<Choice<ExportKindPick>> = draft
        .choices()
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
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", onclick: move |_| oncancel.call(()) }
                    Button { label: "Export", answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                }
            }
        }
    }
}
