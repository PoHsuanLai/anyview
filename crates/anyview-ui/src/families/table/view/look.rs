//! How a sheet looks: the page the rows fill, the title of a sheet, the columns with their kinds
//! and sorts, and the status line.

use super::super::doc::SheetDoc;
use super::super::kind::ColumnKind;
use super::content::{GUTTER, PINNED};
use crate::families::rows::scaled_px;
use crate::{ColumnSort, ContentScale, SortDirection};
use anyview_text::{Coverage, Tally};
use dioxus::prelude::*;
use ds::components::lists::table::model::SortDirection as Way;
use ds::components::lists::table::model::{CellAlign, Sort, Sorting, TableColumn};
use ds::components::lists::virtual_table::row_pitch;
use ds::prelude::Px;

/// How many rows a page up or down moves by: the room's height in rows, less one for the header,
/// so the row the cursor left stays in view.
pub(in crate::families::table) fn page_of(area: Option<crate::Area>, scale: ContentScale) -> u32 {
    area.map_or(1, |area| {
        let rows = (area.size.height.0 / scaled_px(scale)).floor();
        // a count of rows on screen is far below u32's range, and a negative one is none
        (rows as u32).saturating_sub(2).max(1)
    })
}
/// The title of column `at`: the header's cell, or `Column N` when the file has no header.
pub(in crate::families::table) fn title(sheet: &SheetDoc, at: usize) -> String {
    match sheet.table.header() {
        Some(header) => header.get(at).cloned().unwrap_or_default(),
        None => format!("Column {}", at + 1),
    }
}
/// The columns: the row-number gutter, then each column of the sheet at the width its first rows
/// ask for. A character is a quarter of a row's height wide and a cell keeps a row of room around
/// its text (half for the padding, half for a sort arrow in the header), so the widths follow the row token, and the zoom.
pub(super) fn columns_of(sheet: &SheetDoc, scale: ContentScale) -> Vec<TableColumn<u32>> {
    let pitch = row_pitch().0 * scale.factor();
    let gutter = TableColumn::new(GUTTER, "#", Px(pitch * 2.0))
        .aligned(CellAlign::Trailing)
        .sorting(Sorting::Fixed);
    let data = sheet.widths.iter().enumerate().map(|(at, chars)| {
        // Numbers stand at the end, a yes or no's tick and its title in the middle.
        let align = match sheet.kinds.get(at) {
            Some(ColumnKind::Number) => CellAlign::Trailing,
            Some(ColumnKind::YesNo) => CellAlign::Center,
            Some(ColumnKind::Text | ColumnKind::Link | ColumnKind::Tag) | None => {
                CellAlign::Leading
            }
        };
        let id = u32::try_from(at).unwrap_or(GUTTER - 1);
        let width = Px(f32::from(chars.0) * pitch / 4.0 + pitch);
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
/// The class a cell of `kind` takes besides the cell's own.
pub(super) fn kind_class(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Text => "",
        ColumnKind::Number => "viewer-cell-number",
        ColumnKind::YesNo => "viewer-cell-yes-no",
        ColumnKind::Link => "viewer-cell-link",
        ColumnKind::Tag => "viewer-cell-tag",
    }
}
/// What the status line under the table says: how many rows and columns the shown sheet has, or,
/// once several rows are picked, how many (Finder's status bar). A sheet that holds only the start
/// of its file says so with a plus after the rows.
pub(super) fn status_of(sheet: &SheetDoc, rows: u32, picked: usize) -> String {
    if picked > 1 {
        let picked = Tally::Exact(u32::try_from(picked).unwrap_or(u32::MAX));
        return format!("{} rows selected", picked.text());
    }
    let rows = match sheet.table.coverage() {
        Coverage::Whole => Tally::Exact(rows),
        Coverage::Prefix => Tally::AtLeast(rows),
    };
    let columns = sheet.table.columns().0;
    format!(
        "{} {} · {} {}",
        rows.text(),
        if rows == Tally::Exact(1) {
            "row"
        } else {
            "rows"
        },
        Tally::Exact(columns).text(),
        if columns == 1 { "column" } else { "columns" },
    )
}
/// How many of the sheet's columns are held in place: the gutter and the first column, but never
/// all of them (quire always leaves one to scroll).
pub(super) fn held_columns(sheet: &SheetDoc) -> usize {
    PINNED.min(sheet.widths.len())
}
