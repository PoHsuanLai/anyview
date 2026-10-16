//! The stack of pages at the scale on screen: where each is, which part shows, and which tiles
//! to draw.

use super::frame::{Frame, GAP};
use anyview_core::{PageIndex, PageRect, Permille, PixelLen, PixelSize};
use anyview_pdf::{
    PageLayout, PageSize, Schedule, TILE_SIDE, TileKey, ViewWindow, ZoomBucket, device_bound,
    schedule, tile_rect,
};

/// How much of the document is kept ready beyond the room, in rooms.
const PRELOAD_ROOMS: u32 = 1;
/// A rectangle in the stack's own space (left, top, right, bottom), in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::families::pdf) struct Span {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}
impl Span {
    pub(in crate::families::pdf) fn overlaps(self, other: Span) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}
/// The stack of pages at one scale, in a room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::families::pdf) struct Scene {
    pub frame: Frame,
    pub layout: PageLayout,
}
impl Scene {
    pub(in crate::families::pdf) fn new(
        sizes: &[PageSize],
        frame: Frame,
        scale: Permille,
    ) -> Scene {
        Scene {
            frame,
            layout: PageLayout::new(sizes, scale, PixelLen(frame.device(GAP))),
        }
    }

    /// The same stack at another scale.
    pub(in crate::families::pdf) fn at_scale(&self, scale: Permille) -> Scene {
        Scene::new(self.layout.sizes(), self.frame, scale)
    }

    /// The scale the pages are drawn at.
    pub(in crate::families::pdf) fn scale(&self) -> Permille {
        self.layout.scale()
    }

    /// How far down the stack the room can go: the last page's foot at the room's foot, or, when
    /// that page is shorter than the room, its top at the room's top, so a jump to the last page
    /// always lands on it.
    pub(in crate::families::pdf) fn deepest(&self) -> u64 {
        let Some(last) = self
            .layout
            .sizes()
            .len()
            .checked_sub(1)
            .and_then(|at| u32::try_from(at).ok())
            .and_then(|at| self.layout.place(PageIndex(at)))
        else {
            return 0;
        };
        let height = u64::from(last.size.height.0);
        let shown = height.min(u64::from(self.frame.height));
        last.top + height - shown
    }

    /// How far right the room can go; nothing when the pages are narrower than the room.
    pub(in crate::families::pdf) fn widest(&self) -> u32 {
        self.layout.width().saturating_sub(self.frame.width)
    }

    /// The top of the room, in the stack, when it shows `page` `offset` of the way down.
    pub(in crate::families::pdf) fn top_of(&self, page: PageIndex, offset: Permille) -> u64 {
        self.layout.y_of(page, offset).min(self.deepest())
    }

    /// The page at the top of the room and how far down it, for the room's `top`.
    pub(in crate::families::pdf) fn place_at(&self, top: u64) -> (PageIndex, Permille) {
        self.layout.at(top.min(self.deepest()))
    }

    /// Where the stack's left edge is when the room is panned `pan` across: pages narrower than
    /// the room are centred in it, wider ones are panned and kept inside it.
    pub(in crate::families::pdf) fn left_of(&self, pan: u32) -> i64 {
        let spare = i64::from(self.frame.width) - i64::from(self.layout.width());
        if spare >= 0 {
            -(spare / 2)
        } else {
            i64::from(pan.min(self.widest()))
        }
    }

    /// The part of the stack the room shows, for a room whose top is at `top`, panned `pan`.
    pub(in crate::families::pdf) fn view(&self, top: u64, pan: u32) -> Span {
        let left = self.left_of(pan);
        let top = i64::try_from(top).unwrap_or(i64::MAX);
        Span {
            left,
            top,
            right: left + i64::from(self.frame.width),
            bottom: top.saturating_add(i64::from(self.frame.height)),
        }
    }

    /// The tiles the room needs now, and a room's worth of them beyond it.
    pub(in crate::families::pdf) fn schedule(&self, top: u64, pan: u32) -> Schedule {
        let (page, offset) = self.layout.at(top);
        let window = ViewWindow {
            page,
            offset,
            left: i32::try_from(self.left_of(pan)).unwrap_or(0),
            size: PixelSize {
                width: PixelLen(self.frame.width),
                height: PixelLen(self.frame.height),
            },
        };
        let margin = self.frame.width.max(self.frame.height) * PRELOAD_ROOMS / 2;
        schedule(&self.layout, &window, PixelLen(margin))
    }

    /// Where `key`'s pixels go in the stack. Edge tiles are cut to the page.
    pub(in crate::families::pdf) fn tile_span(&self, key: &TileKey) -> Option<Span> {
        let place = self.layout.place(key.page)?;
        let size = self.layout.sizes().get(usize::try_from(key.page.0).ok()?)?;
        let rect = tile_rect(device_bound(*size, key.zoom), key.x, key.y)?;
        let drawn = |pixels: u32| scaled(pixels, self.scale(), key.zoom.scale());
        let (left, top) = (i64::from(place.left), i64::try_from(place.top).ok()?);
        Some(Span {
            left: left + drawn(rect.x),
            top: top + drawn(rect.y),
            right: (left + drawn(rect.x.saturating_add(rect.width)))
                .min(left + i64::from(place.size.width.0)),
            bottom: (top + drawn(rect.y.saturating_add(rect.height)))
                .min(top + i64::from(place.size.height.0)),
        })
    }

    /// The tiles of `zoom` that cover `span` on `page`.
    pub(in crate::families::pdf) fn covering(
        &self,
        page: PageIndex,
        span: Span,
        zoom: ZoomBucket,
    ) -> Vec<TileKey> {
        let (Some(place), Some(size)) = (
            self.layout.place(page),
            self.layout
                .sizes()
                .get(usize::try_from(page.0).unwrap_or(usize::MAX)),
        ) else {
            return Vec::new();
        };
        let bound = device_bound(*size, zoom);
        let (columns, rows) = (
            bound.width.0.div_ceil(TILE_SIDE),
            bound.height.0.div_ceil(TILE_SIDE),
        );
        let (left, top) = (i64::from(place.left), i64::try_from(place.top).unwrap_or(0));
        let first = |edge: i64, origin: i64, count: u32| {
            let at = scaled_back((edge - origin).max(0), self.scale(), zoom.scale());
            (at / TILE_SIDE).min(count)
        };
        let last = |edge: i64, origin: i64, count: u32| {
            let at = scaled_back((edge - origin).max(0), self.scale(), zoom.scale());
            at.div_ceil(TILE_SIDE).min(count)
        };
        let (x0, x1) = (
            first(span.left, left, columns),
            last(span.right, left, columns),
        );
        let (y0, y1) = (first(span.top, top, rows), last(span.bottom, top, rows));
        (y0..y1.max(y0))
            .flat_map(|y| (x0..x1.max(x0)).map(move |x| (x, y)))
            .map(|(x, y)| TileKey { page, zoom, x, y })
            .collect()
    }
}
/// `pixels` drawn at `from` per point, as pixels at `to`, rounded.
fn scaled(pixels: u32, to: Permille, from: Permille) -> i64 {
    let (to, from) = (u64::from(to.0), u64::from(from.0.max(1)));
    i64::try_from((u64::from(pixels) * to + from / 2) / from).unwrap_or(i64::MAX)
}
/// `shown` stack pixels as pixels of a tile grid drawn at `from`, rounded up.
fn scaled_back(shown: i64, at: Permille, from: Permille) -> u32 {
    let (at, from) = (i64::from(at.0.max(1)), i64::from(from.0));
    u32::try_from((shown * from + at - 1) / at).unwrap_or(u32::MAX)
}
/// The tiles to draw, back to front: those of the zoom on screen, and under them any tile of
/// another zoom that shows part of a place the zoom on screen has not drawn yet (the old pixels,
/// scaled, until the new ones arrive). Farther zooms go first so nearer ones lie over them.
pub(in crate::families::pdf) fn drawn(
    scene: &Scene,
    view: Span,
    current: ZoomBucket,
    cached: &[TileKey],
) -> Vec<TileKey> {
    let shows = |key: &TileKey| scene.tile_span(key).is_some_and(|span| span.overlaps(view));
    let have = |key: &TileKey| cached.contains(key);
    let mut under: Vec<TileKey> = cached
        .iter()
        .filter(|key| key.zoom != current && shows(key))
        .filter(|key| {
            scene.tile_span(key).is_some_and(|span| {
                scene
                    .covering(key.page, span, current)
                    .iter()
                    .any(|k| !have(k))
            })
        })
        .copied()
        .collect();
    under.sort_by_key(|key| std::cmp::Reverse(distance(key.zoom, current)));
    let over = cached
        .iter()
        .filter(|key| key.zoom == current && shows(key))
        .copied();
    under.into_iter().chain(over).collect()
}
/// How many steps of the ladder apart two zooms are.
pub(in crate::families::pdf) fn distance(a: ZoomBucket, b: ZoomBucket) -> u32 {
    a.scale().0.abs_diff(b.scale().0)
}
/// A rectangle on a page as CSS percentages (left, top, width, height) of the page's box.
pub(in crate::families::pdf) fn percent(rect: PageRect) -> (f32, f32, f32, f32) {
    let of = |permille: Permille| permille.0 as f32 / 10.0;
    (
        of(rect.left()),
        of(rect.top()),
        of(Permille(rect.width())),
        of(Permille(rect.height())),
    )
}
