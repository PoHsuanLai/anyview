//! A sheet on screen: quire's `VirtualTable`, a header over the rows it mounts. Only the rows
//! near the viewport exist as elements and each is asked of the sheet by `(row, column)`, so a
//! sheet of two hundred thousand rows costs a screenful. A row-number gutter leads the columns;
//! numeric columns sit at the trailing edge; the header's presses sort the rows for the eye
//! only (the stage machine keeps the sort, the file is never reordered).

use super::doc::{SheetDoc, TableDoc};
use crate::families::rows::compact_px;
use crate::families::view::{Held, StageCx};
use crate::{RowNo, SheetNo, Stage, StageIn, TableIn};
use anyview_core::{ColumnSort, SortDirection, sorted_rows};
use anyview_text::RowIndex;
use dioxus::prelude::*;
use ds::components::controls::scroller::handle::use_scroller;
use ds::components::lists::table::model::{
    CellAlign, Sort, SortDirection as Way, Sorting, TableColumn,
};
use ds::components::lists::virtual_table::{VirtualTable, row_pitch};
use ds::components::overlays::empty_state::EmptyState;
use ds::prelude::{Icon, Px};
use std::sync::Arc;

/// The gutter's column id: no column of a file has it.
const GUTTER: u32 = u32::MAX;

/// How many rows a page up or down moves by: the room's height in rows, less one for the header,
/// so the row the cursor left stays in view.
pub(super) fn page_of(area: Option<crate::Area>) -> u32 {
    area.map_or(1, |area| {
        let rows = (area.size.height.0 / compact_px()).floor();
        // a count of rows on screen is far below u32's range, and a negative one is none
        (rows as u32).saturating_sub(2).max(1)
    })
}

/// The title of column `at`: the header's cell, or `Column N` when the file has no header.
fn title(sheet: &SheetDoc, at: usize) -> String {
    match sheet.table.header() {
        Some(header) => header.get(at).cloned().unwrap_or_default(),
        None => format!("Column {}", at + 1),
    }
}

/// The columns: the row-number gutter, then each column of the sheet at the width its first rows
/// ask for. A character is a quarter of a row's height wide and a cell keeps half a row of room
/// around its text, so the widths follow the row token.
fn columns_of(sheet: &SheetDoc) -> Vec<TableColumn<u32>> {
    let pitch = row_pitch().0;
    let gutter = TableColumn::new(GUTTER, "#", Px(pitch * 2.0))
        .aligned(CellAlign::Trailing)
        .sorting(Sorting::Fixed);
    let data = sheet.widths.iter().enumerate().map(|(at, chars)| {
        let align = if sheet.numeric.get(at) == Some(&true) {
            CellAlign::Trailing
        } else {
            CellAlign::Leading
        };
        let id = u32::try_from(at).unwrap_or(GUTTER - 1);
        let width = Px(f32::from(chars.0) * pitch / 4.0 + pitch / 2.0);
        TableColumn::new(id, title(sheet, at), width).aligned(align)
    });
    std::iter::once(gutter).chain(data).collect()
}

/// The sort the machine keeps, as quire's header draws it.
fn drawn(by: ColumnSort) -> Sort<u32> {
    Sort {
        column: by.column,
        direction: match by.direction {
            SortDirection::Ascending => Way::Ascending,
            SortDirection::Descending => Way::Descending,
        },
    }
}

#[component]
pub(super) fn TableContent(doc: Held<TableDoc>, cx: StageCx) -> Element {
    let (sheet_no, by, cursor) = match &cx.stage {
        Stage::Table(stage) => (stage.sheet(), stage.sort(), stage.row()),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => (SheetNo(0), None, None),
    };
    // The hooks come first: a sheet with no rows returns early below, and the next sheet must
    // find the same hooks in the same order.
    let scroller = use_scroller();
    // The rows in the order the sort gives, or `None` for the file's own. Recomputed only when
    // the sheet or the sort changes.
    let order = use_memo(use_reactive!(|doc, sheet_no, by| {
        let sheet = doc.0.sheets.get(sheet_no.0 as usize)?; // a u32 fits a usize
        let by = by?;
        let column = by.column as usize; // a u32 fits a usize
        let numeric = sheet.numeric.get(column) == Some(&true);
        Some(sorted_rows(
            sheet.table.row_count().0,
            by.direction,
            numeric,
            |row| sheet.table.cell(RowIndex(row), column),
        ))
    }));
    // Another file, another order or another sheet starts at the top.
    use_effect(use_reactive!(|doc, sheet_no, by| {
        let _ = (doc, sheet_no, by);
        scroller.scroll_to(Px(0.0));
    }));
    let scrolled = use_memo(move || scroller.scroll().read().offset.0 > 0.0);
    let send = cx.send;
    let sheet: Arc<SheetDoc> = match doc.0.sheets.get(sheet_no.0 as usize) {
        // a u32 fits a usize
        Some(sheet) => Arc::clone(sheet),
        None => return rsx! { div { class: "viewer-data" } },
    };
    let rows = sheet.table.row_count().0 as usize; // a u32 fits a usize
    let held = Arc::clone(&sheet);
    let cell = use_callback(move |(row, column): (usize, usize)| {
        let Some(column) = column.checked_sub(1) else {
            return rsx! {
                span { class: "viewer-row-number", "{row + 1}" }
            };
        };
        let sorted = order.read();
        let file_row = match &*sorted {
            Some(order) => order.get(row).copied(),
            None => None,
        }
        .unwrap_or_else(|| u32::try_from(row).unwrap_or(u32::MAX));
        let text = held.table.cell(RowIndex(file_row), column);
        rsx! { "{text}" }
    });
    if rows == 0 {
        return rsx! {
            div { class: "viewer-data",
                EmptyState { icon: Icon::File, title: "This sheet has no rows" }
            }
        };
    }
    let shown = if scrolled() { "yes" } else { "no" };
    rsx! {
        div {
            class: "viewer-data",
            "data-body": "table",
            "data-scrolled": shown,
            VirtualTable::<u32> {
                label: "Rows",
                columns: columns_of(&sheet),
                rows,
                cell,
                scroller,
                sort: by.map(drawn),
                on_sort: move |asked: Sort<u32>| {
                    send.call(StageIn::Table(TableIn::PressHeader(asked.column)));
                },
                cursor: cursor.map(|row| row.0 as usize), // a u32 fits a usize
                onselect: move |row: usize| {
                    let row = RowNo(u32::try_from(row).unwrap_or(u32::MAX));
                    send.call(StageIn::Table(TableIn::Select(row)));
                },
            }
        }
    }
}
