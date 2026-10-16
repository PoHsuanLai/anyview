//! A hit or a point of the pointer as a place in the stack of pages.

use super::stack::Scene;
use anyview_core::{PageIndex, PagePoint, PageRect};

impl Scene {
    /// The top of the room that shows a hit's first rectangle a third of the way down, never
    /// above the hit's own page, so a hit on a page is always shown with that page on top.
    pub(in crate::families::pdf) fn top_for_hit(
        &self,
        page: PageIndex,
        rects: &[PageRect],
    ) -> Option<u64> {
        let place = self.layout.place(page)?;
        let first = rects.first()?;
        let hit = place.top + u64::from(place.size.height.0) * u64::from(first.top().0) / 1000;
        let top = hit
            .saturating_sub(u64::from(self.frame.height) / 3)
            .max(place.top);
        Some(top.min(self.deepest()))
    }

    /// How far to pan so a hit's first rectangle is in the middle of the room, when the pages
    /// are wider than the room.
    pub(in crate::families::pdf) fn pan_for_hit(&self, page: PageIndex, rects: &[PageRect]) -> u32 {
        let (Some(place), Some(first)) = (self.layout.place(page), rects.first()) else {
            return 0;
        };
        let across = u64::from(place.size.width.0) * u64::from(first.left().0) / 1000;
        let at = u64::from(place.left) + across;
        let pan = at.saturating_sub(u64::from(self.frame.width) / 2);
        u32::try_from(pan).unwrap_or(u32::MAX).min(self.widest())
    }

    /// The top and pan of a room at the scale of `self` that keeps the stack point under `at`
    /// (a point of the room) where it was under it in `before`'s room at `top` and `pan`.
    pub(in crate::families::pdf) fn anchored(
        &self,
        before: &Scene,
        top: u64,
        pan: u32,
        at: (u32, u32),
    ) -> (u64, u32) {
        let ratio = f64::from(self.scale().0) / f64::from(before.scale().0.max(1));
        let held = (
            (before.left_of(pan) + i64::from(at.0)) as f64 * ratio,
            (i64::try_from(top).unwrap_or(i64::MAX) + i64::from(at.1)) as f64 * ratio,
        );
        let top = (held.1 - f64::from(at.1)).max(0.0) as u64;
        let pan = (held.0 - f64::from(at.0)).max(0.0) as u32;
        (top.min(self.deepest()), pan.min(self.widest()))
    }
}
impl Scene {
    /// The page under `device`, a point of the room in device pixels (the room's top at `top`,
    /// panned `pan`), and where on it; `None` for a point in a gap or beside the pages.
    pub(in crate::families::pdf) fn page_point(
        &self,
        top: u64,
        pan: u32,
        device: (u32, u32),
    ) -> Option<(PageIndex, PagePoint)> {
        let y = top.saturating_add(u64::from(device.1));
        let (page, _) = self.layout.at(y);
        let place = self.layout.place(page)?;
        let across = self.left_of(pan) + i64::from(device.0) - i64::from(place.left);
        let down = i64::try_from(y).ok()? - i64::try_from(place.top).ok()?;
        let (width, height) = (
            i64::from(place.size.width.0),
            i64::from(place.size.height.0),
        );
        if across < 0 || down < 0 || across > width || down > height {
            return None;
        }
        Some((page, self.point_on(page, top, pan, device)))
    }

    /// Where `device` is on `page`, held on it: the pointer of a drag that left the page is at its
    /// edge.
    pub(in crate::families::pdf) fn point_on(
        &self,
        page: PageIndex,
        top: u64,
        pan: u32,
        device: (u32, u32),
    ) -> PagePoint {
        let Some(place) = self.layout.place(page) else {
            return PagePoint::default();
        };
        let y = i64::try_from(top).unwrap_or(i64::MAX) + i64::from(device.1);
        let across = self.left_of(pan) + i64::from(device.0) - i64::from(place.left);
        let down = y - i64::try_from(place.top).unwrap_or(i64::MAX);
        let thousandths = |at: i64, length: u32| {
            let whole = (at * 1000 / i64::from(length.max(1))).clamp(0, 1000);
            u32::try_from(whole).unwrap_or(0)
        };
        PagePoint::at(
            thousandths(across, place.size.width.0),
            thousandths(down, place.size.height.0),
        )
    }
}
