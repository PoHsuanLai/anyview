//! A sheet on screen: quire's `VirtualTable`, a header over the rows it mounts. Only the rows
//! near the viewport exist as elements and each is asked of the sheet by `(row, column)`, which
//! parses the window of the file it is in, so a sheet of any size costs a screenful. A row-number gutter leads the columns;
//! numeric columns sit at the trailing edge; the header's presses sort the rows for the eye
//! only (the stage machine keeps the sort, the file is never reordered).

use super::doc::{SheetDoc, TableDoc};
use crate::families::rows::scaled_px;
use crate::families::view::{Held, StageCx};
use crate::families::{Mark, cut};
use crate::io::Job;
use crate::{
    ColumnSort, ContentScale, HitIndex, RowNo, SheetNo, SortDirection, Stage, StageIn, TableIn,
};
use anyview_text::RowIndex;
use dioxus::prelude::*;
use ds::components::controls::scroller::handle::use_scroller;
use ds::components::lists::table::model::{
    CellAlign, Sort, SortDirection as Way, Sorting, TableColumn,
};
use ds::components::lists::virtual_table::{VirtualTable, row_pitch};
use ds::components::overlays::empty_state::EmptyState;
use ds::host::document::{DocumentHost, use_document_host};
use ds::host::found::Found;
use ds::host::measure::client_rect;
use ds::host::reveal::{ScrollSpan, nearest_scroll};
use ds::prelude::{ColumnRules, Common, Icon, Px};
use ds::style::busy::wait_out_busy;
use ds_blitz::{ElementId, ScrollAnimate, ScrollAxis, ScrollCmd, ScrollHandle, use_scroll_handle};
use std::rc::Rc;
use std::sync::Arc;

/// The gutter's column id: no column of a file has it.
const GUTTER: u32 = u32::MAX;

/// The id of the table's box, which scrolls sideways when the columns are wider than the room.
const TABLE_ID: &str = "viewer-table";

/// The room the table fills: it does not scroll, so its rect is where the table shows.
const ROOM: &str = "[data-body=\"table\"]";

/// The cell the current hit is in: its text is marked apart.
const CURRENT_HIT: &str = "[data-body=\"table\"] .viewer-hit-current";

/// How many times a hit's cell is looked for: it exists once its row is mounted, a frame or two
/// after the cursor moved to the row.
const LOOKS: usize = 8;

/// Scroll the table sideways the least that shows the cell of the current hit. Quire's scroller
/// moves the rows up and down; the columns wider than the room are the table's own box, which is
/// moved by a command. Call it from a task.
async fn show_across(host: Rc<dyn DocumentHost>, scroll: ScrollHandle) {
    for attempt in 0..LOOKS {
        wait_out_busy(attempt).await;
        let (Found::Element(room), Found::Element(cell)) = (
            host.geometry().find(ROOM),
            host.geometry().find(CURRENT_HIT),
        ) else {
            continue;
        };
        let (Some(room), Some(cell)) = (client_rect(&room).await, client_rect(&cell).await) else {
            continue;
        };
        if cell.size.width.0 <= 0.0 {
            continue;
        }
        // Both rects are the window's; the room is where the table shows and does not scroll.
        let span = ScrollSpan {
            start: cell.origin.x.0 - room.origin.x.0,
            length: cell.size.width.0,
        };
        let by = nearest_scroll(0.0, room.size.width.0, span);
        if by != 0.0 {
            scroll.send(ScrollCmd::By {
                element: ElementId::new(TABLE_ID),
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
        let align = if sheet.numeric.get(at) == Some(&true) {
            CellAlign::Trailing
        } else {
            CellAlign::Leading
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

#[component]
pub(super) fn TableContent(doc: Held<TableDoc>, cx: StageCx) -> Element {
    let (sheet_no, by, cursor, scale) = match &cx.stage {
        Stage::Table(stage) => (stage.sheet(), stage.sort(), stage.row(), stage.scale()),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => (SheetNo(0), None, None, ContentScale::ACTUAL),
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
    let held = Arc::clone(&sheet);
    let marked = cx.hits.clone();
    let cell = use_callback(move |(row, column): (usize, usize)| {
        let Some(column) = column.checked_sub(1) else {
            return rsx! {
                span { class: "viewer-row-number viewer-cell", "{row + 1}" }
            };
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
        let pieces = cut(&text, &marks);
        rsx! {
            span { class: "viewer-cell",
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
                rules: ColumnRules::Hairline,
                common: Common {
                    id: Some(TABLE_ID.to_owned()),
                    ..Common::default()
                },
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
