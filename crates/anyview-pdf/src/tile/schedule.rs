//! Which tiles a view needs, in the order to draw them: the tiles under the viewport first, nearest
//! the middle first, then the ring around it (a margin all round, so the next page or the next
//! screen of a zoomed page is ready before it is scrolled to).

use super::key::{TILE_SIDE, TileKey, TileRect, device_bound, tile_grid, tile_rect};
use super::zoom::ZoomBucket;
use crate::layout::{PageLayout, PagePlace};
use anyview_core::{PageIndex, Permille, PixelLen, PixelSize};

/// The part of the stack the window shows: where its top edge is (a page and how far down it) and
/// how far its left edge is from the left of the stack, with its size. All in pixels at the
/// layout's scale. `left` is negative when the stack is narrower than the window and centred in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewWindow {
    /// The page at the top of the window.
    pub page: PageIndex,
    /// How far down that page the window starts.
    pub offset: Permille,
    /// Distance from the left of the stack to the window's left edge.
    pub left: i32,
    /// The window's size.
    pub size: PixelSize,
}

/// Whether a tile is on screen or only near it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Priority {
    /// Under the window: drawn first.
    Visible,
    /// In the margin around it: drawn when nothing visible waits.
    Preload,
}

/// A tile's column and row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    /// Column.
    pub x: u32,
    /// Row.
    pub y: u32,
}

/// The tiles of one page at one zoom, drawn by one job: the page is read once for all of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileBatch {
    /// The page.
    pub page: PageIndex,
    /// The scale it is drawn at.
    pub zoom: ZoomBucket,
    /// Whether the batch is on screen.
    pub priority: Priority,
    /// The tiles, nearest the middle of the window first. Never empty.
    pub tiles: Vec<TileCoord>,
}

/// The tiles a view needs, each list nearest the middle of the window first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    zoom: ZoomBucket,
    visible: Vec<TileKey>,
    preload: Vec<TileKey>,
}

/// A rectangle in the stack's pixels; the stack can be left of the window, so signed.
#[derive(Debug, Clone, Copy)]
struct Area {
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
}

impl Area {
    fn overlaps(self, other: Area) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    fn grown(self, by: i64) -> Area {
        Area {
            left: self.left - by,
            top: self.top - by,
            right: self.right + by,
            bottom: self.bottom + by,
        }
    }
}

/// The tiles that `window` needs of the stack: those under it, and those within `margin` pixels of
/// it. The zoom bucket is the lowest that draws at the layout's scale or larger.
pub fn schedule(layout: &PageLayout, window: &ViewWindow, margin: PixelLen) -> Schedule {
    let zoom = ZoomBucket::containing(layout.scale());
    let top = i64::try_from(layout.y_of(window.page, window.offset)).unwrap_or(i64::MAX);
    let shown = Area {
        left: i64::from(window.left),
        top,
        right: i64::from(window.left) + i64::from(window.size.width.0),
        bottom: top.saturating_add(i64::from(window.size.height.0)),
    };
    let near = shown.grown(i64::from(margin.0));
    let middle = (
        (shown.left + shown.right) / 2,
        (shown.top + shown.bottom) / 2,
    );
    let from = u64::try_from(near.top).unwrap_or(0);
    let to = u64::try_from(near.bottom).unwrap_or(0);

    let (mut visible, mut preload) = (Vec::new(), Vec::new());
    for (page, place) in layout.between(from, to) {
        let Some(size) = layout.sizes().get(page.0 as usize) else {
            continue;
        };
        let device = device_bound(*size, zoom);
        for (x, y) in tiles_in(place, near, layout, zoom, device) {
            let area = tile_area(place, layout, zoom, device, x, y);
            let distance = (area.left + area.right) / 2 - middle.0;
            let distance = distance.abs() + ((area.top + area.bottom) / 2 - middle.1).abs();
            let key = TileKey { page, zoom, x, y };
            if area.overlaps(shown) {
                visible.push((distance, key));
            } else {
                preload.push((distance, key));
            }
        }
    }
    Schedule {
        zoom,
        visible: nearest_first(visible),
        preload: nearest_first(preload),
    }
}

fn nearest_first(mut tiles: Vec<(i64, TileKey)>) -> Vec<TileKey> {
    tiles.sort_by_key(|(distance, key)| (*distance, key.page, key.y, key.x));
    tiles.into_iter().map(|(_, key)| key).collect()
}

/// A page-local coordinate at the layout's scale as a pixel of the zoom bucket, rounded down.
fn drawn_floor(shown: i64, layout: &PageLayout, zoom: ZoomBucket) -> u32 {
    let scaled = shown.max(0) * i64::from(zoom.scale().0) / i64::from(layout.scale().0.max(1));
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// The same, rounded up.
fn drawn_ceil(shown: i64, layout: &PageLayout, zoom: ZoomBucket) -> u32 {
    let (from, to) = (
        i64::from(zoom.scale().0),
        i64::from(layout.scale().0.max(1)),
    );
    let scaled = (shown.max(0) * from + to - 1) / to;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// A page-local coordinate of the zoom bucket as one at the layout's scale, rounded to the nearest.
fn shown_at(drawn: u32, layout: &PageLayout, zoom: ZoomBucket) -> i64 {
    let (to, from) = (
        i64::from(layout.scale().0),
        i64::from(zoom.scale().0.max(1)),
    );
    (i64::from(drawn) * to + from / 2) / from
}

/// The columns and rows of the page's tiles that `area` (in the stack's pixels) touches.
fn tiles_in(
    place: &PagePlace,
    area: Area,
    layout: &PageLayout,
    zoom: ZoomBucket,
    device: PixelSize,
) -> impl Iterator<Item = (u32, u32)> {
    let (columns, rows) = tile_grid(device);
    let page_left = i64::from(place.left);
    let page_top = i64::try_from(place.top).unwrap_or(i64::MAX);
    let across = |from: i64, to: i64, count: u32| {
        let (lo, hi) = (
            drawn_floor(from, layout, zoom),
            drawn_ceil(to, layout, zoom),
        );
        let first = (lo / TILE_SIDE).min(count);
        let last = hi.div_ceil(TILE_SIDE).min(count);
        first..last.max(first)
    };
    let xs = across(area.left - page_left, area.right - page_left, columns);
    let ys = across(area.top - page_top, area.bottom - page_top, rows);
    ys.flat_map(move |y| xs.clone().map(move |x| (x, y)))
}

/// Where a tile is in the stack's pixels.
fn tile_area(
    place: &PagePlace,
    layout: &PageLayout,
    zoom: ZoomBucket,
    device: PixelSize,
    x: u32,
    y: u32,
) -> Area {
    let rect = tile_rect(device, x, y).unwrap_or(TileRect {
        x: x.saturating_mul(TILE_SIDE),
        y: y.saturating_mul(TILE_SIDE),
        width: TILE_SIDE,
        height: TILE_SIDE,
    });
    let (left, top) = (
        i64::from(place.left),
        i64::try_from(place.top).unwrap_or(i64::MAX),
    );
    Area {
        left: left + shown_at(rect.x, layout, zoom),
        top: top + shown_at(rect.y, layout, zoom),
        // The edge tile is cut to the page as laid out: the device bound is a pixel generous.
        right: (left + shown_at(rect.x.saturating_add(rect.width), layout, zoom))
            .min(left + i64::from(place.size.width.0)),
        bottom: (top + shown_at(rect.y.saturating_add(rect.height), layout, zoom))
            .min(top + i64::from(place.size.height.0)),
    }
}

impl Schedule {
    /// The zoom bucket every tile here is drawn at.
    pub fn zoom(&self) -> ZoomBucket {
        self.zoom
    }

    /// The tiles under the window, nearest its middle first.
    pub fn visible(&self) -> &[TileKey] {
        &self.visible
    }

    /// The tiles in the margin, nearest the window's middle first.
    pub fn preload(&self) -> &[TileKey] {
        &self.preload
    }

    /// Every tile wanted, visible ones first.
    pub fn keys(&self) -> impl Iterator<Item = &TileKey> {
        self.visible.iter().chain(&self.preload)
    }

    /// Whether `key` is wanted at all: a cache keeps what this says yes to.
    pub fn wants(&self, key: &TileKey) -> bool {
        self.keys().any(|wanted| wanted == key)
    }

    /// What is still to draw: the schedule without the tiles `have` says are drawn.
    pub fn missing(&self, have: impl Fn(&TileKey) -> bool) -> Schedule {
        let lacking = |keys: &[TileKey]| keys.iter().filter(|key| !have(key)).copied().collect();
        Schedule {
            zoom: self.zoom,
            visible: lacking(&self.visible),
            preload: lacking(&self.preload),
        }
    }

    /// The jobs to run, in order: one batch per page, the visible ones first. Within a batch the
    /// tiles keep their order, and a page's batches are ordered by its nearest tile.
    pub fn batches(&self) -> Vec<TileBatch> {
        let mut batches = Vec::new();
        for (priority, keys) in [
            (Priority::Visible, &self.visible),
            (Priority::Preload, &self.preload),
        ] {
            let first = batches.len();
            for key in keys {
                let coord = TileCoord { x: key.x, y: key.y };
                match batches[first..]
                    .iter_mut()
                    .find(|batch: &&mut TileBatch| batch.page == key.page)
                {
                    Some(batch) => batch.tiles.push(coord),
                    None => batches.push(TileBatch {
                        page: key.page,
                        zoom: key.zoom,
                        priority,
                        tiles: vec![coord],
                    }),
                }
            }
        }
        batches
    }
}
