//! A sheet on screen: quire's `VirtualTable`, a header over the rows it mounts. Only the rows
//! near the viewport exist as elements and each is asked of the sheet by `(row, column)`, which
//! parses the window of the file it is in, so a sheet of any size costs a screenful. A row-number gutter leads the columns;
//! numeric columns sit at the trailing edge; the header's presses sort the rows for the eye
//! only (the stage machine keeps the sort, the file is never reordered). The gutter and the
//! first column are held in place while the others scroll sideways under them, and rows are
//! picked as in Finder's list: a click, Command-click, Shift-click, Shift and the arrows, Select
//! All, with Copy putting them on the clipboard as tab-separated lines (`pick`).

use super::SortedOrder;
use super::doc::{SheetDoc, TableDoc};
use super::kind::{ColumnKind, is_yes};
use super::pick::reseat;
use crate::families::rows::scaled_px;
use crate::families::view::{Held, StageCx};
use crate::families::{Mark, cut};
use crate::io::Job;
use crate::{ColumnSort, ContentScale, HitIndex, SheetNo, SortDirection, Stage, StageIn, TableIn};
use anyview_text::{Coverage, RowIndex, Tally};
use dioxus::prelude::*;
use ds::components::controls::scroller::handle::use_scroller;
use ds::components::lists::table::model::{
    CellAlign, Sort, SortDirection as Way, Sorting, TableColumn,
};
use ds::components::lists::virtual_table::{VirtualTable, columns_id, row_pitch};
use ds::components::overlays::empty_state::EmptyState;
use ds::host::document::{DocumentHost, use_document_host};
use ds::host::found::Found;
use ds::host::measure::client_rect;
use ds::host::reveal::{ScrollSpan, nearest_scroll};
use ds::prelude::{Common, Icon, Px, TableLook};
use ds::style::busy::wait_out_busy;
use ds_blitz::{ElementId, ScrollAnimate, ScrollAxis, ScrollCmd, ScrollHandle, use_scroll_handle};
use ds_core::selection::{Pick, RowSelection};
use std::rc::Rc;
use std::sync::Arc;

/// The gutter's column id: no column of a file has it.
const GUTTER: u32 = u32::MAX;

/// The id of the table's box, which scrolls sideways when the columns are wider than the room.
const TABLE_ID: &str = "viewer-table";

/// How many of the leading columns are held in place while the others scroll sideways: the
/// row-number gutter and the sheet's first column.
const PINNED: usize = 2;

/// The room the table fills: it does not scroll, so its rect is where the table shows.
const ROOM: &str = "[data-body=\"table\"]";

/// The columns held in place, which the columns that scroll start beside.
const HELD: &str = "[data-body=\"table\"] .ds-vtable-held";

/// The cell the current hit is in: its text is marked apart.
const CURRENT_HIT: &str = "[data-body=\"table\"] .viewer-hit-current";

/// The current hit's cell when it is in a column that is held in place, which is always in view.
const CURRENT_HELD_HIT: &str = "[data-body=\"table\"] .viewer-held .viewer-hit-current";

/// How many times a hit's cell is looked for: it exists once its row is mounted, a frame or two
/// after the cursor moved to the row.
const LOOKS: usize = 8;

/// Scroll the table sideways the least that shows the cell of the current hit, clear of the
/// columns that are held in place. Quire's scroller moves the rows up and down; the columns wider
/// than the room are the table's own box, which is moved by a command. A hit in a held column
/// needs no scroll. Call it from a task.
async fn show_across(host: Rc<dyn DocumentHost>, scroll: ScrollHandle) {
    let columns = columns_id(TABLE_ID);
    for attempt in 0..LOOKS {
        wait_out_busy(attempt).await;
        match host.geometry().find(CURRENT_HELD_HIT) {
            Found::Element(_) => return,
            Found::Busy => continue,
            Found::Missing | Found::BadSelector | Found::Unreachable => {}
        }
        let (Found::Element(table), Found::Element(held), Found::Element(cell)) = (
            host.geometry().find(ROOM),
            host.geometry().find(HELD),
            host.geometry().find(CURRENT_HIT),
        ) else {
            continue;
        };
        let (Some(table), Some(held), Some(cell)) = (
            client_rect(&table).await,
            client_rect(&held).await,
            client_rect(&cell).await,
        ) else {
            continue;
        };
        if cell.size.width.0 <= 0.0 {
            continue;
        }
        // All three rects are the window's. The room is the columns that scroll: from where the
        // held columns end to the table's right edge. The scrolling box's own rect is not used,
        // since Blitz moves a scrolled box's rect by its own scroll.
        let start = held.origin.x.0 + held.size.width.0;
        let span = ScrollSpan {
            start: cell.origin.x.0 - start,
            length: cell.size.width.0,
        };
        let by = nearest_scroll(0.0, table.origin.x.0 + table.size.width.0 - start, span);
        if by != 0.0 {
            scroll.send(ScrollCmd::By {
                element: ElementId::new(&columns),
                axis: ScrollAxis::X,
                delta: f64::from(by),
                animate: ScrollAnimate::Instant,
            });
        }
        return;
    }
}

/// How many rows a page up or down moves by: the room's height in rows, less one for the header,
/// so the row the cursor left stays in view.
pub(super) fn page_of(area: Option<crate::Area>, scale: ContentScale) -> u32 {
    area.map_or(1, |area| {
        let rows = (area.size.height.0 / scaled_px(scale)).floor();
        // a count of rows on screen is far below u32's range, and a negative one is none
        (rows as u32).saturating_sub(2).max(1)
    })
}

/// The title of column `at`: the header's cell, or `Column N` when the file has no header.
pub(super) fn title(sheet: &SheetDoc, at: usize) -> String {
    match sheet.table.header() {
        Some(header) => header.get(at).cloned().unwrap_or_default(),
        None => format!("Column {}", at + 1),
    }
}

/// The columns: the row-number gutter, then each column of the sheet at the width its first rows
/// ask for. A character is a quarter of a row's height wide and a cell keeps a row of room around
/// its text (half for the padding, half for a sort arrow in the header), so the widths follow the row token, and the zoom.
fn columns_of(sheet: &SheetDoc, scale: ContentScale) -> Vec<TableColumn<u32>> {
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
fn kind_class(kind: ColumnKind) -> &'static str {
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
fn status_of(sheet: &SheetDoc, picked: usize) -> String {
    if picked > 1 {
        let picked = Tally::Exact(u32::try_from(picked).unwrap_or(u32::MAX));
        return format!("{} rows selected", picked.text());
    }
    let rows = sheet.table.row_count().0;
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
fn held_columns(sheet: &SheetDoc) -> usize {
    PINNED.min(sheet.widths.len())
}

#[component]
pub(super) fn TableContent(doc: Held<TableDoc>, cx: StageCx) -> Element {
    let (sheet_no, by, cursor, picked, scale) = match &cx.stage {
        Stage::Table(stage) => (
            stage.sheet(),
            stage.sort(),
            stage.row(),
            stage.selection().cloned(),
            stage.scale(),
        ),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => (SheetNo(0), None, None, None, ContentScale::ACTUAL),
    };
    // The hit the find is on, so its cell is marked apart from the others.
    let current = cx.stage.find_state().and_then(|(_, hits)| hits.current());
    // The hooks come first: a sheet with no rows returns early below, and the next sheet must
    // find the same hooks in the same order.
    let scroller = use_scroller();
    // The hit's cell may be in a column that is off to the side: it is brought into the room
    // whenever the hit changes (not when the person scrolls the columns themselves).
    let host = use_document_host();
    let sideways = use_scroll_handle();
    use_effect(use_reactive!(|current| {
        if let (Some(_), Some(scroll)) = (current, sideways.clone()) {
            spawn(show_across(Rc::clone(&host), scroll));
        }
    }));
    // A sort is made on a worker: its header shows at once and the rows swap to the new order
    // when it arrives. Until then they stay as they are.
    let work = cx.work;
    let ticket = cx.ticket;
    use_effect(use_reactive!(|doc, sheet_no, by| {
        if let Some(sort) = by {
            work.call(Job::Sort {
                ticket,
                doc: Arc::clone(&doc.0),
                sheet: sheet_no,
                sort,
            });
        }
    }));
    // The rows in the order the last sort of this sheet gave, or `None` for the file's own.
    let sorted = cx.sorted.clone();
    let order = use_memo(use_reactive!(|by, sheet_no, sorted| {
        by?;
        let held = sorted?;
        (held.0.sheet == sheet_no).then(|| Arc::clone(&held.0))
    }));
    // The rows picked are places in the order shown. When that order changes (a sort arrived, or
    // was put away) the same rows of the file stay picked: the machine is told their new places.
    let mut seated = use_signal(|| None::<Arc<SortedOrder>>);
    let mut latest = use_hook(|| CopyValue::new((None::<RowSelection>, 0usize)));
    let send_places = cx.send;
    use_effect(move || {
        let now = order();
        let before = seated.peek().clone();
        let same = match (&before, &now) {
            (None, None) => true,
            (Some(before), Some(now)) => Arc::ptr_eq(before, now),
            (None, Some(_)) | (Some(_), None) => false,
        };
        if same {
            return;
        }
        seated.set(now.clone());
        let (Some(rows), total) = latest.peek().clone() else {
            return;
        };
        let from = before.as_ref().map(|order| order.order.as_slice());
        let to = now.as_ref().map(|order| order.order.as_slice());
        let moved = reseat(&rows, total, from, to);
        if moved != rows {
            send_places.call(StageIn::Table(TableIn::Reseat(moved)));
        }
    });
    // Another file, another order or another sheet starts at the top.
    use_effect(use_reactive!(|doc, sheet_no, by| {
        let _ = (doc, sheet_no, by);
        scroller.scroll_to(Px(0.0));
    }));
    // A zoom changes how tall a row is: the row at the top stays the row at the top.
    let mut zoomed_from = use_signal(|| scale);
    use_effect(use_reactive!(|scale| {
        let before = *zoomed_from.peek();
        if before != scale {
            zoomed_from.set(scale);
            let offset = scroller.scroll().peek().offset;
            scroller.scroll_to(Px(offset.0 * scale.factor() / before.factor()));
        }
    }));
    let send = cx.send;
    let sheet: Arc<SheetDoc> = match doc.0.sheets.get(sheet_no.0 as usize) {
        // a u32 fits a usize
        Some(sheet) => Arc::clone(sheet),
        None => return rsx! { div { class: "viewer-data viewer-scrolls" } },
    };
    let rows = sheet.table.row_count().0 as usize; // a u32 fits a usize
    latest.set((picked.clone(), rows));
    let kept = held_columns(&sheet);
    let held = Arc::clone(&sheet);
    let marked = cx.hits.clone();
    let cell = use_callback(move |(row, column): (usize, usize)| {
        let Some(column) = column.checked_sub(1) else {
            let held_class = if kept > 0 { " viewer-held" } else { "" };
            return rsx! {
                span { class: "viewer-row-number viewer-cell{held_class}", "{row + 1}" }
            };
        };
        // A cell in a column that is held in place is marked, so the find knows it is in view.
        let held_class = if column + 1 < kept {
            " viewer-held"
        } else {
            ""
        };
        let sorted = order.read();
        let file_row = match &*sorted {
            Some(sorted) => sorted.order.get(row).copied(),
            None => None,
        }
        .unwrap_or_else(|| u32::try_from(row).unwrap_or(u32::MAX));
        let text = held.table.cell(RowIndex(file_row), column);
        let marks: Vec<(std::ops::Range<usize>, Mark)> = match marked.as_ref() {
            Some(found) => {
                let (first, on_row) = found.0.in_row(file_row);
                on_row
                    .iter()
                    .enumerate()
                    .filter(|(_, hit)| hit.column as usize == column) // a u32 fits a usize
                    .map(|(at, hit)| {
                        let this = HitIndex(first.0 + u32::try_from(at).unwrap_or(u32::MAX));
                        let mark = if current == Some(this) {
                            Mark::Current
                        } else {
                            Mark::Hit
                        };
                        (hit.from as usize..hit.to as usize, mark) // a u32 fits a usize
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        let kind = held.kinds.get(column).copied().unwrap_or_default();
        if text.trim().is_empty() {
            return rsx! {
                span { class: "viewer-cell viewer-cell-empty{held_class}", "—" }
            };
        }
        // A yes is a tick and a no is nothing, unless a find is on the cell: then its text shows,
        // so the hit can be seen and found.
        if kind == ColumnKind::YesNo && marks.is_empty() {
            let tick = if is_yes(&text) { "✓" } else { "" };
            return rsx! {
                span { class: "viewer-cell viewer-cell-yes-no{held_class}", "{tick}" }
            };
        }
        let pieces = cut(&text, &marks);
        let class = format!("viewer-cell {}{held_class}", kind_class(kind));
        rsx! {
            span { class,
                for (piece , mark) in pieces {
                    span { class: mark.class(), "{piece}" }
                }
            }
        }
    });
    if rows == 0 {
        return rsx! {
            div { class: "viewer-data viewer-scrolls",
                EmptyState { icon: Icon::File, title: "This sheet has no rows" }
            }
        };
    }
    let status = status_of(&sheet, picked.as_ref().map_or(0, RowSelection::count));
    rsx! {
        div {
            class: "viewer-data viewer-scrolls",
            "data-body": "table",
            style: "--zoom:{scale.factor()}",
            VirtualTable::<u32> {
                label: "Rows",
                columns: columns_of(&sheet, scale),
                pitch: Px(row_pitch().0 * scale.factor()),
                rows,
                cell,
                scroller,
                // Numbers' look: a hairline between the columns, and one under the header once
                // the rows are scrolled (quire's default).
                look: TableLook::Striped,
                common: Common {
                    id: Some(TABLE_ID.to_owned()),
                    ..Common::default()
                },
                sort: by.map(drawn),
                on_sort: move |asked: Sort<u32>| {
                    send.call(StageIn::Table(TableIn::PressHeader(asked.column)));
                },
                cursor: cursor.map(|row| row.0 as usize), // a u32 fits a usize
                selection: picked,
                // A click, Command-click, Shift-click or an arrow with Shift says how the row
                // was reached, and the machine keeps the rows picked.
                onreach: move |pick: Pick| {
                    send.call(StageIn::Table(TableIn::Pick(pick)));
                },
                pinned: PINNED,
            }
            p { class: "viewer-table-status", "{status}" }
        }
    }
}
