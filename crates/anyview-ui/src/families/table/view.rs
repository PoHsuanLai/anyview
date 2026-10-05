//! A sheet on screen: a header over the rows a `VirtualList` mounts. Only the rows near the
//! viewport exist as elements, so a sheet of a hundred thousand rows costs a screenful.

use super::doc::{SheetDoc, TableDoc};
use crate::families::view::{Held, StageCx};
use crate::{RowNo, Stage, StageIn, TableIn};
use anyview_text::RowIndex;
use dioxus::prelude::*;
use ds::components::lists::row::row::Row;
use ds::components::lists::row::size::RowSize;
use ds::components::lists::virtual_list::{RowHeight, VirtualList};
use ds::components::overlays::empty_state::EmptyState;
use ds::prelude::{Icon, Px};
use ds_core::vocab::{RowState, Selection};
use std::sync::Arc;

/// How wide a character is, for sizing a column.
const CHAR_PX: f32 = 8.0;
/// Room a cell keeps around its text.
const CELL_PAD_PX: f32 = 16.0;
/// The row-number column's width.
const NUMBER_PX: f32 = 56.0;
/// The height of a compact row (`RowSize::Compact`).
const ROW_PX: f32 = 24.0;

/// `grid-template-columns`: the row-number column, then each column its width.
fn template(sheet: &SheetDoc) -> (String, f32) {
    let widths: Vec<f32> = sheet
        .widths
        .iter()
        .map(|chars| f32::from(chars.0) * CHAR_PX + CELL_PAD_PX)
        .collect();
    let total = NUMBER_PX + widths.iter().sum::<f32>();
    let columns = widths
        .iter()
        .map(|width| format!("{width}px"))
        .collect::<Vec<_>>()
        .join(" ");
    (format!("{NUMBER_PX}px {columns}"), total)
}

/// The title of column `at`: the header's cell, or `Column N` when the file has no header.
fn title(sheet: &SheetDoc, at: usize) -> String {
    match sheet.table.header() {
        Some(header) => header.get(at).cloned().unwrap_or_default(),
        None => format!("Column {}", at + 1),
    }
}

fn row_cells(sheet: &SheetDoc, index: u32) -> Element {
    let cells = sheet.widths.len();
    let number = index + 1;
    rsx! {
        span { class: "ds-table-cells",
            span { class: "ds-table-cell ds-truncate viewer-row-number", "data-align": "trailing", "{number}" }
            for column in 0..cells {
                span { key: "{column}", class: "ds-table-cell ds-truncate",
                    {sheet.table.cell(RowIndex(index), column).to_owned()}
                }
            }
        }
    }
}

#[component]
pub(super) fn TableContent(doc: Held<TableDoc>, cx: StageCx) -> Element {
    let Stage::Table(stage) = &cx.stage else {
        return rsx! { div { class: "viewer-data" } };
    };
    let sheet: Arc<SheetDoc> = match doc.0.sheets.get(stage.sheet().0 as usize) {
        // a u32 fits a usize
        Some(sheet) => Arc::clone(sheet),
        None => return rsx! { div { class: "viewer-data" } },
    };
    let rows = sheet.table.row_count().0;
    if rows == 0 {
        return rsx! {
            div { class: "viewer-data",
                EmptyState { icon: Icon::File, title: "This sheet has no rows" }
            }
        };
    }
    let (columns, width) = template(&sheet);
    let cursor = stage.row().map(|row| row.0);
    let send = cx.send;
    let drawn = Arc::clone(&sheet);
    let row = use_callback(move |index: u32| {
        let state = RowState {
            selection: if Some(index) == cursor {
                Selection::Selected
            } else {
                Selection::Unselected
            },
            ..RowState::default()
        };
        rsx! {
            Row {
                title: "",
                size: RowSize::Compact,
                state,
                content: Some(row_cells(&drawn, index)),
                onclick: move |_| send.call(StageIn::Table(TableIn::Select(RowNo(index)))),
            }
        }
    });
    let keys: Vec<u32> = (0..rows).collect();
    let note = (sheet.table.coverage() == anyview_text::Coverage::Prefix)
        .then(|| format!("Showing the first {rows} rows"));
    let sheet_key = stage.sheet().0;
    rsx! {
        div { class: "viewer-data", "data-body": "table",
            div { class: "viewer-data-scroll",
                div {
                    class: "ds-table viewer-data-table",
                    style: "--table-cols:{columns};min-width:{width}px",
                    div { class: "ds-table-header", role: "row",
                        span { class: "ds-table-head",
                            span { class: "ds-table-column", "data-align": "trailing",
                                span { class: "ds-table-title", "#" }
                            }
                        }
                        for column in 0..sheet.widths.len() {
                            span { key: "{column}", class: "ds-table-head",
                                span { class: "ds-table-column",
                                    span { class: "ds-table-title", {title(&sheet, column)} }
                                }
                            }
                        }
                    }
                    VirtualList::<u32> {
                        key: "{sheet_key}",
                        label: "Rows",
                        keys,
                        row,
                        height: RowHeight::Fixed(Px(ROW_PX)),
                        cursor,
                        onselect: move |index: u32| send.call(StageIn::Table(TableIn::Select(RowNo(index)))),
                    }
                }
            }
            if let Some(note) = note {
                p { class: "viewer-data-note", "{note}" }
            }
        }
    }
}
