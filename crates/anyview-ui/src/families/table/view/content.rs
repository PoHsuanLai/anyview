//! A sheet on screen: quire's `VirtualTable` and what the pointer and keys do in it.

use super::super::SortedOrder;
use super::super::doc::{SheetDoc, TableDoc};
use super::super::edits::{row_at, text_at};
use super::super::kind::{ColumnKind, is_yes};
use super::super::pick::reseat;
use super::look::{columns_of, drawn, held_columns, kind_class, status_of};
use super::scroll::show_across;
use crate::families::inline_edit::InlineEdit;
use crate::families::view::{Held, StageCx};
use crate::families::{Mark, cut};
use crate::io::Job;
use crate::views::press::CHORD_KEYS;
use crate::{
    AfterTyping, CellFocus, Cells, ColumnNo, ContentScale, HitIndex, RowNo, SheetNo, Stage,
    StageIn, TableIn, TypingIn, TypingSeed, TypingState,
};
use anyview_text::{RowIndex, RowRef};
use dioxus::html::ModifiersInteraction as _;
use dioxus::prelude::*;
use ds::components::controls::scroller::handle::use_scroller;
use ds::components::lists::table::model::Sort;
use ds::components::lists::virtual_table::{VirtualTable, row_pitch};
use ds::components::overlays::empty_state::EmptyState;
use ds::host::document::use_document_host;
use ds::prelude::{Common, Icon, Px, TableLook};
use ds_blitz::use_scroll_handle;
use ds_core::selection::{Pick, RowSelection};
use std::rc::Rc;
use std::sync::Arc;

/// The gutter's column id: no column of a file has it.
pub(super) const GUTTER: u32 = u32::MAX;
/// The id of the table's box, which scrolls sideways when the columns are wider than the room.
pub(super) const TABLE_ID: &str = "viewer-table";
/// How many of the leading columns are held in place while the others scroll sideways: the
/// row-number gutter and the sheet's first column.
pub(super) const PINNED: usize = 2;
/// The room the table fills: it does not scroll, so its rect is where the table shows.
pub(super) const ROOM: &str = "[data-body=\"table\"]";
/// The columns held in place, which the columns that scroll start beside.
pub(super) const HELD: &str = "[data-body=\"table\"] .ds-vtable-held";
/// The cell the current hit is in: its text is marked apart.
pub(super) const CURRENT_HIT: &str = "[data-body=\"table\"] .viewer-hit-current";
/// The current hit's cell when it is in a column that is held in place, which is always in view.
pub(super) const CURRENT_HELD_HIT: &str = "[data-body=\"table\"] .viewer-held .viewer-hit-current";
/// How many times a hit's cell is looked for: it exists once its row is mounted, a frame or two
/// after the cursor moved to the row.
pub(super) const LOOKS: usize = 8;
#[component]
pub(crate) fn TableContent(doc: Held<TableDoc>, cx: StageCx) -> Element {
    let (sheet_no, by, cursor, picked, scale, cells_now) = match &cx.stage {
        Stage::Table(stage) => (
            stage.sheet(),
            stage.sort(),
            stage.row(),
            stage.selection().cloned(),
            stage.scale(),
            stage.cells().clone(),
        ),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => (
            SheetNo(0),
            None,
            None,
            None,
            ContentScale::ACTUAL,
            Cells::default(),
        ),
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
    let rows_now = doc.0.rows_now(sheet_no.0 as usize); // a u32 fits a usize
    let rows = rows_now as usize; // a u32 fits a usize
    latest.set((picked.clone(), rows));
    // The rows are drawn again when the edits change, a cell is typed in or the cell cursor moves,
    // though the rows picked stay: the cells read this.
    let mut shown = use_signal(|| (cells_now.clone(), cursor));
    use_effect(use_reactive!(|cells_now, cursor| {
        shown.set((cells_now, cursor));
    }));
    let editable = doc.0.is_editable();
    let typing = use_callback(move |input: TypingIn| {
        send.call(StageIn::Table(TableIn::Typing(input)));
    });
    let sheet: Arc<SheetDoc> = match doc.0.sheets.get(sheet_no.0 as usize) {
        // a u32 fits a usize
        Some(sheet) => Arc::clone(sheet),
        None => return rsx! { div { class: "viewer-data viewer-scrolls" } },
    };
    let kept = held_columns(&sheet);
    let held = Arc::clone(&sheet);
    let marked = cx.hits.clone();
    let table_doc = Arc::clone(&doc.0);
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
        let (cells, at_cursor) = shown.read().clone();
        let sorted = order.read();
        let order_slice: Option<&[u32]> = match &*sorted {
            Some(order) => Some(order.order.as_slice()),
            None => None,
        };
        // The cell as the edits leave it, and the row of the file it is in for the find's marks.
        let (text, file_row) = match table_doc.edits().as_ref() {
            Some(live) => {
                let file = match row_at(live, order_slice, row) {
                    Some(RowRef::File(file)) => Some(file),
                    Some(RowRef::New(_)) | None => None,
                };
                (text_at((live, &held.table), order_slice, row, column), file)
            }
            None => {
                let file = order_slice
                    .and_then(|order| order.get(row).copied())
                    .unwrap_or_else(|| u32::try_from(row).unwrap_or(u32::MAX));
                (held.table.cell(RowIndex(file), column), Some(file))
            }
        };
        let column_no = ColumnNo(u32::try_from(column).unwrap_or(u32::MAX));
        let row_no = RowNo(u32::try_from(row).unwrap_or(u32::MAX));
        let here = at_cursor == Some(row_no) && cells.focus == CellFocus::Cell(column_no);
        let focus_class = if here { " viewer-cell-focus" } else { "" };
        if here && let TypingState::Value(seed) = &cells.edits.typing {
            let start = match seed {
                TypingSeed::Existing => text.clone(),
                TypingSeed::Typed(typed) => typed.as_str().to_owned(),
            };
            return rsx! {
                span { class: "viewer-cell viewer-cell-editing{held_class}",
                    InlineEdit {
                        seed: start,
                        on_return: AfterTyping::Down,
                        on_tab: Some(AfterTyping::Right),
                        send: typing,
                    }
                }
            };
        }
        // A click puts the cell cursor on the cell; a double-click types into it.
        let reach = move |event: MouseEvent| {
            if editable
                && !event
                    .modifiers()
                    .intersects(CHORD_KEYS.union(Modifiers::SHIFT))
            {
                send.call(StageIn::Table(TableIn::FocusCell(row_no, column_no)));
            }
        };
        let edit_here = move |_: MouseEvent| {
            if editable {
                send.call(StageIn::Table(TableIn::EditAt(row_no, column_no)));
            }
        };
        let marks: Vec<(std::ops::Range<usize>, Mark)> = match (marked.as_ref(), file_row) {
            (Some(found), Some(file_row)) => {
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
            (Some(_) | None, Some(_) | None) => Vec::new(),
        };
        let kind = held.kinds.get(column).copied().unwrap_or_default();
        if text.trim().is_empty() {
            return rsx! {
                span {
                    class: "viewer-cell viewer-cell-empty{held_class}{focus_class}",
                    onclick: reach,
                    ondoubleclick: edit_here,
                    "—"
                }
            };
        }
        // A yes is a tick and a no is nothing, unless a find is on the cell: then its text shows,
        // so the hit can be seen and found.
        if kind == ColumnKind::YesNo && marks.is_empty() {
            let tick = if is_yes(&text) { "✓" } else { "" };
            return rsx! {
                span {
                    class: "viewer-cell viewer-cell-yes-no{held_class}{focus_class}",
                    onclick: reach,
                    ondoubleclick: edit_here,
                    "{tick}"
                }
            };
        }
        let pieces = cut(&text, &marks);
        let class = format!("viewer-cell {}{held_class}{focus_class}", kind_class(kind));
        rsx! {
            span { class, onclick: reach, ondoubleclick: edit_here,
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
    let status = status_of(
        &sheet,
        rows_now,
        picked.as_ref().map_or(0, RowSelection::count),
    );
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
