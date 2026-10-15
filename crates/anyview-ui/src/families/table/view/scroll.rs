//! Bringing the current hit of a find into view across the held rows and the scrolling ones.

use super::content::{CURRENT_HELD_HIT, CURRENT_HIT, HELD, LOOKS, ROOM, TABLE_ID};
use dioxus::html::ModifiersInteraction as _;
use dioxus::prelude::*;
use ds::components::lists::virtual_table::columns_id;
use ds::host::document::DocumentHost;
use ds::host::found::Found;
use ds::host::measure::client_rect;
use ds::host::reveal::{ScrollSpan, nearest_scroll};
use ds::style::busy::wait_out_busy;
use ds_blitz::{ElementId, ScrollAnimate, ScrollAxis, ScrollCmd, ScrollHandle};
use std::rc::Rc;

/// Scroll the table sideways the least that shows the cell of the current hit, clear of the
/// columns that are held in place. Quire's scroller moves the rows up and down; the columns wider
/// than the room are the table's own box, which is moved by a command. A hit in a held column
/// needs no scroll. Call it from a task.
pub(super) async fn show_across(host: Rc<dyn DocumentHost>, scroll: ScrollHandle) {
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
