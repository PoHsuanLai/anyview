//! The export dialog: it appears in place, with the formats listed on one side, each with a line
//! of what it is for, and the chosen format's options on the other.

use super::export_options::option_row;
use super::sheet::confirms;
use crate::{
    ExportDraft, ExportFacts, ExportKindPick, ExportOption, MediaOffer, PageSpan, kind_hint,
    kind_name,
};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::button_model::Answers;
use ds::components::lists::list::model::ListStyle;
use ds::components::overlays::sheet_attach::Attach;
use ds::prelude::{Button, Form, FormSection, List, ListItem, Row, Sheet};
use ds_core::vocab::{RowState, Selection};
use ds_core::word::Word;

/// The list of formats: a row each, with its hint under its name, the chosen one selected. The
/// arrow keys move through it as through any list.
fn formats(
    kinds: Vec<ExportKindPick>,
    chosen: ExportKindPick,
    onpick: EventHandler<ExportKindPick>,
) -> Element {
    let items: Vec<ListItem<ExportKindPick>> = kinds
        .into_iter()
        .map(|kind| {
            let state = RowState {
                selection: if kind == chosen {
                    Selection::Selected
                } else {
                    Selection::Unselected
                },
                ..RowState::default()
            };
            ListItem::row(
                kind,
                kind_name(kind),
                rsx! {
                    Row {
                        title: kind_name(kind),
                        detail: Some(TextLine::from(kind_hint(kind))),
                        state,
                        onclick: move |_| onpick.call(kind),
                    }
                },
            )
        })
        .collect();
    rsx! {
        List::<ExportKindPick> {
            label: "Format",
            items,
            style: ListStyle::Grouped,
            cursor: Some(chosen),
            onselect: move |kind| onpick.call(kind),
            onpick: move |kind| onpick.call(kind),
        }
    }
}

/// Choosing what to export, in a dialog that appears in place: the formats listed on one side,
/// the chosen format's options on the other (stacked when the dialog is narrow), then the note of
/// where the file will be, Cancel and Export. Only the media formats `offer` holds are listed;
/// when more would be there with another package, the dialog says which.
#[component]
pub(super) fn ExportSheet(
    draft: ExportDraft,
    span: PageSpan,
    facts: ExportFacts,
    offer: MediaOffer,
    name: String,
    onpick: EventHandler<ExportKindPick>,
    ontune: EventHandler<ExportOption>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    let kinds = draft.choices_within(&offer);
    let pick = draft.pick();
    let controls = draft.controls(&facts, span);
    let saved = draft.saved_as(&name);
    rsx! {
        Sheet {
            label: "Export",
            attach: Attach::Centre,
            onclose: move |()| oncancel.call(()),
            div { class: "viewer-sheet viewer-export", onkeydown: move |event| confirms(&event, onconfirm),
                div { class: "viewer-export-body",
                    div { class: "viewer-export-formats", {formats(kinds, pick, onpick)} }
                    div { class: "viewer-export-options",
                        if controls.is_empty() {
                            p { class: "viewer-export-hint", "There is nothing to set for {kind_name(pick)}." }
                        } else {
                            Form {
                                FormSection { title: kind_name(pick).to_owned(),
                                    for control in controls {
                                        {option_row(control, draft, span, facts, ontune)}
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(needs) = offer.needs() {
                    p { class: "viewer-sheet-note viewer-export-needs",
                        "{needs.label.label()}: {needs.value.as_str()}"
                    }
                }
                div { class: "viewer-export-foot",
                    if let Some(saved) = saved {
                        p { class: "viewer-sheet-note viewer-export-saved", "Saved beside the original as {saved}" }
                    }
                    div { class: "viewer-sheet-buttons",
                        Button { label: "Cancel", answers: Answers::Escape, onclick: move |_| oncancel.call(()) }
                        Button { label: "Export", answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                    }
                }
            }
        }
    }
}
