//! Where the pages are in the room: the stack of pages at the scale on screen, what scale each zoom
//! means for this window, which part of the stack the room shows and what that asks of the tile
//! scheduler. Pure arithmetic over integers: a float appears only where a length becomes CSS.
//!
//! Every length here is a device pixel and every scale is device pixels per point, so `Actual`
//! (1000) draws a point on one device pixel, as the raster stage draws a texel.

use crate::Area;
use anyview_core::{PageIndex, Permille, PixelLen, PixelSize, Zoom};
use anyview_pdf::{
    PageLayout, PageRect, PageSize, Schedule, TILE_SIDE, TileKey, ViewWindow, ZoomBucket,
    device_bound, schedule, tile_rect,
};

/// The gap between two pages, in logical pixels.
pub(super) const GAP: u32 = 12;

/// The room left around a page that is fitted, in logical pixels.
pub(super) const MARGIN: u32 = 16;

/// How much of the document is kept ready beyond the room, in rooms.
const PRELOAD_ROOMS: u32 = 1;

/// The room in device pixels, and how many device pixels a logical one is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Frame {
    pub width: u32,
    pub height: u32,
    /// Device pixels per logical pixel, in thousandths.
    pub density: Permille,
}

impl Frame {
    /// The room `area` measured.
    pub(super) fn of(area: Area) -> Frame {
        let density = per_mille(f64::from(area.scale)).max(1);
        let device = |logical: f32| per_mille(f64::from(logical) * f64::from(area.scale)) / 1000;
        Frame {
            width: device(area.size.width.0).max(1),
            height: device(area.size.height.0).max(1),
            density: Permille(density),
        }
    }

    /// `logical` pixels as device pixels.
    pub(super) fn device(self, logical: u32) -> u32 {
        u32::try_from(u64::from(logical) * u64::from(self.density.0) / 1000).unwrap_or(u32::MAX)
    }

    /// `device` pixels as the logical pixels CSS counts.
    pub(super) fn css(self, device: i64) -> f32 {
        device as f32 * 1000.0 / self.density.0.max(1) as f32
    }
}

fn per_mille(value: f64) -> u32 {
    // A scale or a length is positive and far below 2^32 / 1000, so the cast cannot wrap.
    (value * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// The two scales a zoom can mean for this document in this room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Fits {
    /// The widest page fills the room's width.
    pub width: Permille,
    /// The largest page fits the room whole.
    pub page: Permille,
}

/// The scales at which `sizes` fit `frame`: the margin is kept on every side.
pub(super) fn fits(sizes: &[PageSize], frame: Frame) -> Fits {
    let wide = sizes
        .iter()
        .map(|size| size.width.0)
        .max()
        .unwrap_or(1)
        .max(1);
    let tall = sizes
        .iter()
        .map(|size| size.height.0)
        .max()
        .unwrap_or(1)
        .max(1);
    let margin = 2 * frame.device(MARGIN);
    let across = |room: u32, points: u32| {
        let room = u64::from(room.saturating_sub(margin).max(1));
        let scale = room * 1_000_000 / u64::from(points);
        Permille(u32::try_from(scale).unwrap_or(u32::MAX))
    };
    let width = across(frame.width, wide);
    let page = Permille(width.0.min(across(frame.height, tall).0));
    Fits {
        width: within_limits(width),
        page: within_limits(page),
    }
}

/// `scale` kept inside the zoom limits.
fn within_limits(scale: Permille) -> Permille {
    Permille(scale.0.clamp(Zoom::MIN_SCALE.0, Zoom::MAX_SCALE.0))
}

/// The scale `zoom` draws at.
pub(super) fn scale_of(zoom: Zoom, fits: Fits) -> Permille {
    match zoom {
        Zoom::Fit => fits.page,
        Zoom::Fill => fits.width,
        Zoom::Actual => Permille::WHOLE,
        Zoom::Scale(scale) => within_limits(scale),
    }
}

/// The scale after a pinch or a wheel turn of `by` thousandths, kept inside the zoom limits.
pub(super) fn pinched(shown: Permille, by: i32) -> Permille {
    let grown = i64::from(shown.0) * (1000 + i64::from(by.clamp(-900, 4000))) / 1000;
    within_limits(Permille(u32::try_from(grown).unwrap_or(Zoom::MIN_SCALE.0)))
}

/// A rectangle in the stack's own space (left, top, right, bottom), in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Span {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

impl Span {
    pub(super) fn overlaps(self, other: Span) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

/// The stack of pages at one scale, in a room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Scene {
    pub frame: Frame,
    pub layout: PageLayout,
}

impl Scene {
    pub(super) fn new(sizes: &[PageSize], frame: Frame, scale: Permille) -> Scene {
        Scene {
            frame,
            layout: PageLayout::new(sizes, scale, PixelLen(frame.device(GAP))),
        }
    }

    /// The same stack at another scale.
    pub(super) fn at_scale(&self, scale: Permille) -> Scene {
        Scene::new(self.layout.sizes(), self.frame, scale)
    }

    /// The scale the pages are drawn at.
    pub(super) fn scale(&self) -> Permille {
        self.layout.scale()
    }

    /// How far down the stack the room can go: the last page's foot at the room's foot, or, when
    /// that page is shorter than the room, its top at the room's top, so a jump to the last page
    /// always lands on it.
    pub(super) fn deepest(&self) -> u64 {
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
    pub(super) fn widest(&self) -> u32 {
        self.layout.width().saturating_sub(self.frame.width)
    }

    /// The top of the room, in the stack, when it shows `page` `offset` of the way down.
    pub(super) fn top_of(&self, page: PageIndex, offset: Permille) -> u64 {
        self.layout.y_of(page, offset).min(self.deepest())
    }

    /// The page at the top of the room and how far down it, for the room's `top`.
    pub(super) fn place_at(&self, top: u64) -> (PageIndex, Permille) {
        self.layout.at(top.min(self.deepest()))
    }

    /// Where the stack's left edge is when the room is panned `pan` across: pages narrower than
    /// the room are centred in it, wider ones are panned and kept inside it.
    pub(super) fn left_of(&self, pan: u32) -> i64 {
        let spare = i64::from(self.frame.width) - i64::from(self.layout.width());
        if spare >= 0 {
            -(spare / 2)
        } else {
            i64::from(pan.min(self.widest()))
        }
    }

    /// The part of the stack the room shows, for a room whose top is at `top`, panned `pan`.
    pub(super) fn view(&self, top: u64, pan: u32) -> Span {
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
    pub(super) fn schedule(&self, top: u64, pan: u32) -> Schedule {
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
    pub(super) fn tile_span(&self, key: &TileKey) -> Option<Span> {
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
    pub(super) fn covering(&self, page: PageIndex, span: Span, zoom: ZoomBucket) -> Vec<TileKey> {
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
pub(super) fn drawn(
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
pub(super) fn distance(a: ZoomBucket, b: ZoomBucket) -> u32 {
    a.scale().0.abs_diff(b.scale().0)
}

/// A rectangle on a page as CSS percentages (left, top, width, height) of the page's box.
pub(super) fn percent(rect: PageRect) -> (f32, f32, f32, f32) {
    let of = |permille: Permille| permille.0 as f32 / 10.0;
    (
        of(rect.left),
        of(rect.top),
        of(Permille(rect.right.0.saturating_sub(rect.left.0))),
        of(Permille(rect.bottom.0.saturating_sub(rect.top.0))),
    )
}

impl Scene {
    /// The top of the room that shows a hit's first rectangle a third of the way down, never
    /// above the hit's own page, so a hit on a page is always shown with that page on top.
    pub(super) fn top_for_hit(&self, page: PageIndex, rects: &[PageRect]) -> Option<u64> {
        let place = self.layout.place(page)?;
        let first = rects.first()?;
        let hit = place.top + u64::from(place.size.height.0) * u64::from(first.top.0) / 1000;
        let top = hit
            .saturating_sub(u64::from(self.frame.height) / 3)
            .max(place.top);
        Some(top.min(self.deepest()))
    }

    /// How far to pan so a hit's first rectangle is in the middle of the room, when the pages
    /// are wider than the room.
    pub(super) fn pan_for_hit(&self, page: PageIndex, rects: &[PageRect]) -> u32 {
        let (Some(place), Some(first)) = (self.layout.place(page), rects.first()) else {
            return 0;
        };
        let across = u64::from(place.size.width.0) * u64::from(first.left.0) / 1000;
        let at = u64::from(place.left) + across;
        let pan = at.saturating_sub(u64::from(self.frame.width) / 2);
        u32::try_from(pan).unwrap_or(u32::MAX).min(self.widest())
    }

    /// The top and pan of a room at the scale of `self` that keeps the stack point under `at`
    /// (a point of the room) where it was under it in `before`'s room at `top` and `pan`.
    pub(super) fn anchored(
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
