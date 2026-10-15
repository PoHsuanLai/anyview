//! A pointer over the pages: what a press means, how a drag is carried, and the room's pixels.

use crate::MarkupIn;
use crate::families::view::Area;
use anyview_core::PageIndex;
use dioxus::prelude::*;
use ds::prelude::Point;
use ds_core::word::Word;

/// What a drag with the primary button does on the pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(super) enum PointerUse {
    /// It moves the pages.
    Panning,
    /// It selects the words under it.
    Selecting,
    /// It draws, picks and moves marks.
    Marking,
}
/// A drag that is not a pan: the page it went down on, and where the words it selects begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dragging {
    /// Words being selected from the one at `anchor`.
    Words { page: PageIndex, anchor: usize },
    /// A shape being drawn, or a mark being moved.
    Shape { page: PageIndex },
}
/// A point of the window as a point of the room, in device pixels; a point above or left of the
/// room is its edge.
pub(super) fn room_device(at: (f32, f32), area: Area) -> (u32, u32) {
    let across = |value: f32, origin: f32| u32::try_from(device(value - origin, area)).unwrap_or(0);
    (across(at.0, area.origin.x.0), across(at.1, area.origin.y.0))
}
/// How far a drag may travel and still be a click on a link, in logical pixels.
pub(super) const CLICK_SLOP: f32 = 6.0;
pub(super) fn over(area: Area, at: Point) -> bool {
    let (x, y) = (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0);
    x >= 0.0 && y >= 0.0 && x < area.size.width.0 && y < area.size.height.0
}
/// A distance in logical pixels as device pixels.
pub(super) fn device(logical: f32, area: Area) -> i64 {
    // A pointer distance is far inside i64, so rounding to whole pixels cannot overflow.
    (logical * area.scale).round() as i64
}
/// `logical` and the `carried` remainder as whole device pixels, and the remainder left over.
pub(super) fn whole_pixels(logical: f32, carried: f32, area: Area) -> (i64, f32) {
    let total = logical * area.scale + carried;
    let whole = total.round();
    (whole as i64, total - whole)
}
/// A point of the window as a point of the room, in device pixels.
pub(super) fn in_room(at: Point, area: Area) -> (u32, u32) {
    room_device((at.x.0, at.y.0), area)
}
/// The pointer was let go after a drag that selects or draws: words drawn over with the tool that
/// marks words are marked, and a shape is made.
pub(super) fn released(
    drag: Dragging,
    pointer: PointerUse,
    tool: anyview_core::MarkTool,
    mark: EventHandler<MarkupIn>,
) {
    match drag {
        Dragging::Words { .. } => {
            if pointer == PointerUse::Marking {
                mark.call(MarkupIn::Apply(tool));
            }
        }
        Dragging::Shape { .. } => mark.call(MarkupIn::Release),
    }
}
