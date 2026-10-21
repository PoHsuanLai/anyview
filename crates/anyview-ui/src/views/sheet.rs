//! The modal sheets: rename, save a copy, revert to a version, the trash question and the install question. The sheet machine says which is open
//! and what it holds (the export dialog is `export`); these draw them and report the person's choices as sheet inputs.

use crate::{EditCaution, HelperPhase, HelperWords, TypedText, VersionKey, VersionList, Wanted};
use anyview_core::{Edit, Fact, Helper};
use dioxus::prelude::*;
use ds::components::controls::button_model::Answers;
use ds::components::controls::copy_state::CopyState;
use ds::components::overlays::alert_model::{AlertButton, AlertRole};
use ds::prelude::{Alert, Button, Choice, FieldFocus, RadioGroup, Sheet, TextField};
use ds_core::word::Word;
use ds_shell::helpers::{HelperPhase as Sheeted, HelperSheet};

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

/// Another program changed the file while its text was edited: Replace writes the text over what
/// it wrote, Cancel keeps the person where they were.
#[component]
pub(super) fn ReplaceSheet(
    name: String,
    onreplace: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    rsx! {
        Alert {
            title: format!("\u{201c}{name}\u{201d} was changed by another application."),
            message: Some("Saving will replace those changes with yours.".to_owned().into()),
            buttons: vec![
                AlertButton::new("Replace", AlertRole::Normal, onreplace),
                AlertButton::new("Cancel", AlertRole::Cancel, oncancel),
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
        (Edit::Markup, EditCaution::Signed) => ("Change this document?", "Save"),
        (_, EditCaution::Signed) => ("Change this document?", "Change Pages"),
        (Edit::Flip(_), EditCaution::Loses(_)) => ("Flip this picture?", "Flip"),
        (Edit::Adjust(_), EditCaution::Loses(_)) => ("Save the changes to this picture?", "Save"),
        (Edit::Trim(_), EditCaution::Loses(_)) => ("Trim this recording?", "Trim"),
        (_, EditCaution::Loses(_)) => ("Rotate this picture?", "Rotate"),
    };
    let message = match caution {
        EditCaution::Signed if edit == Edit::Markup => {
            "This document is signed. Saving your changes may make its signature invalid."
                .to_owned()
        }
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
        Sheet { label, onclose: move |()| oncancel.call(()), on_return: onconfirm,
            div { class: "viewer-sheet",
                TextField {
                    label: "Name",
                    value: name.as_str().to_owned(),
                    focus: FieldFocus::OnMount,
                    oninput: move |text: String| ontyped.call(TypedText::new(text)),
                }
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", answers: Answers::Escape, onclick: move |_| oncancel.call(()) }
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
        Sheet { label: "Revert To", onclose: move |()| oncancel.call(()), on_return: onconfirm,
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
                    Button { label: "Cancel", answers: Answers::Escape, onclick: move |_| oncancel.call(()) }
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
        Sheet { label: "Revert To", onclose: move |()| onclose.call(()), on_return: onclose,
            div { class: "viewer-sheet",
                p { class: "viewer-sheet-note", "This file has no earlier version." }
                div { class: "viewer-sheet-buttons",
                    Button { label: "OK", answers: Answers::Return, onclick: move |_| onclose.call(()) }
                }
            }
        }
    }
}

/// An export or a trim the viewer cannot offer at all: which package adds it. Enter and Esc put
/// it away.
#[component]
pub(super) fn UnavailableSheet(
    needs: Fact,
    helper: Option<Helper>,
    wanted: Wanted,
    onclose: EventHandler<()>,
    oninstall: EventHandler<Helper>,
) -> Element {
    let (label, note) = match wanted {
        Wanted::Export => ("Export", "There is nothing to export yet."),
        Wanted::Trim => ("Trim", "This recording can\u{2019}t be trimmed yet."),
    };
    rsx! {
        Sheet { label, onclose: move |()| onclose.call(()), on_return: onclose,
            div { class: "viewer-sheet",
                p { class: "viewer-sheet-note", "{note}" }
                p { class: "viewer-sheet-note",
                    "{needs.label.label()}: {needs.value.as_str()}"
                }
                div { class: "viewer-sheet-buttons",
                    if let Some(helper) = helper {
                        Button { label: "Install…", onclick: move |_| oninstall.call(helper) }
                    }
                    Button { label: "OK", answers: Answers::Return, onclick: move |_| onclose.call(()) }
                }
            }
        }
    }
}

/// Asking to install a tool the open file needs, then following the install: quire's sheet, whose
/// phase the machine holds. Return installs and Esc is Not Now, as the sheet's own keys say.
#[component]
pub(super) fn InstallSheet(
    words: HelperWords,
    phase: HelperPhase,
    oninstall: EventHandler<()>,
    ondismiss: EventHandler<()>,
) -> Element {
    let HelperWords { app, tool, purpose } = words;
    // The sheet says "Copied" once the command is on the clipboard; a clipboard that cannot be
    // written leaves the button as it was, as a copy in a text does.
    let mut copy = use_signal(|| CopyState::Idle);
    let phase = match phase {
        HelperPhase::Ask => Sheeted::Ask,
        HelperPhase::Installing => Sheeted::Installing,
        HelperPhase::Failed(reason) => Sheeted::Failed { reason },
        HelperPhase::NotFound(package) => Sheeted::NotFound { package },
        HelperPhase::Unsupported { program, command } => Sheeted::Unsupported { program, command },
    };
    rsx! {
        HelperSheet {
            app,
            tool,
            purpose,
            phase,
            copy: copy(),
            on_copy: move |command: String| {
                if ds_blitz::clipboard::write_text(&command).is_ok() {
                    copy.set(CopyState::Copied);
                }
            },
            on_install: move |()| oninstall.call(()),
            on_dismiss: move |()| ondismiss.call(()),
        }
    }
}
