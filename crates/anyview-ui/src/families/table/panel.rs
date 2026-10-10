//! The side panel's part for a table: the Sheets tab (a row each, the open one selected), and the
//! note under it and under the Info card when the file is longer than what is shown. The note is
//! the panel's footer, never a banner over the rows.

use super::doc::TableDoc;
use crate::families::view::StageCx;
use crate::{PanelTab, SheetNo, Stage, StageIn, TableIn};
use anyview_text::Coverage;
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::prelude::Row;
use ds_core::vocab::{RowState, Selection};
use std::sync::Arc;

/// The panel's body for `tab`, when the file has more than its facts to show there.
pub(super) fn body(doc: &Arc<TableDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
    let note = footer(doc, cx);
    match tab {
        PanelTab::Sheets => Some(rsx! {
            {sheets(doc, cx)}
            if let Some(note) = note {
                {note}
            }
        }),
        PanelTab::Info => note,
        PanelTab::Thumbnails | PanelTab::Contents | PanelTab::Tracks => None,
    }
}

fn current(cx: &StageCx) -> SheetNo {
    match &cx.stage {
        Stage::Table(stage) => stage.sheet(),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => SheetNo(0),
    }
}

/// How the panel says the open sheet holds only the start of its file.
fn footer(doc: &Arc<TableDoc>, cx: &StageCx) -> Option<Element> {
    let sheet = doc.sheets.get(current(cx).0 as usize)?; // a u32 fits a usize
    match sheet.table.coverage() {
        Coverage::Whole => None,
        Coverage::Prefix => {
            let text = format!("Showing the first {} rows", sheet.table.row_count().0);
            Some(rsx! {
                p { class: "viewer-panel-note", "{text}" }
            })
        }
    }
}

fn sheets(doc: &Arc<TableDoc>, cx: &StageCx) -> Element {
    let current = current(cx);
    let send = cx.send;
    rsx! {
        div { class: "viewer-sheets",
            for (at , sheet) in doc.sheets.iter().enumerate() {
                {
                    let no = SheetNo(u32::try_from(at).unwrap_or(u32::MAX));
                    let state = RowState {
                        selection: if no == current { Selection::Selected } else { Selection::Unselected },
                        ..RowState::default()
                    };
                    rsx! {
                        Row {
                            key: "{at}",
                            state,
                            title: sheet.name.clone(),
                            detail: Some(TextLine::from(format!("{} rows", sheet.table.row_count().0))),
                            onclick: move |_| send.call(StageIn::Table(TableIn::ChooseSheet(no))),
                        }
                    }
                }
            }
        }
    }
}
