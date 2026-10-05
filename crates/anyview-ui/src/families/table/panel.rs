//! The side panel's list of a workbook's sheets: a row each, the open one selected.

use super::doc::TableDoc;
use crate::families::view::StageCx;
use crate::{PanelTab, SheetNo, Stage, StageIn, TableIn};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::prelude::Row;
use ds_core::vocab::{RowState, Selection};
use std::sync::Arc;

/// The panel's body for `tab`, when the file has more than its facts to show there.
pub(super) fn body(doc: &Arc<TableDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
    match tab {
        PanelTab::Contents => Some(sheets(doc, cx)),
        PanelTab::Info | PanelTab::Thumbnails | PanelTab::Tracks => None,
    }
}

fn sheets(doc: &Arc<TableDoc>, cx: &StageCx) -> Element {
    let current = match &cx.stage {
        Stage::Table(stage) => stage.sheet(),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Book(_)
        | Stage::Tree(_) => SheetNo(0),
    };
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
