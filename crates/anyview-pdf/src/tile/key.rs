//! What a tile is: a square of one page drawn at one zoom bucket, named by its column and row.

use super::zoom::ZoomBucket;
use crate::geometry::PageSize;
use crate::layout::pixels;
use anyview_core::{PageIndex, PixelLen, PixelSize};

/// The side of a tile in pixels. Tiles on the right and bottom edges of a page are smaller.
pub const TILE_SIDE: u32 = 512;

/// One tile of one page at one zoom bucket: the key a tile cache is indexed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    /// The page.
    pub page: PageIndex,
    /// The scale it is drawn at.
    pub zoom: ZoomBucket,
    /// The tile's column, from the left of the page.
    pub x: u32,
    /// The tile's row, from the top of the page.
    pub y: u32,
}

/// A tile's place and size on its page, in pixels at its zoom bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRect {
    /// Pixels from the left of the page.
    pub x: u32,
    /// Pixels from the top of the page.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl TileKey {
    /// The pixel at the tile's top left corner, from the top left of the page.
    pub fn origin(&self) -> (PixelLen, PixelLen) {
        (
            PixelLen(self.x.saturating_mul(TILE_SIDE)),
            PixelLen(self.y.saturating_mul(TILE_SIDE)),
        )
    }
}

/// The tile in column `x`, row `y` of a page `device` pixels across, or `None` when it starts past
/// the page's edge. A tile that crosses the edge is cut to it.
pub fn tile_rect(device: PixelSize, x: u32, y: u32) -> Option<TileRect> {
    let (left, top) = (x.checked_mul(TILE_SIDE)?, y.checked_mul(TILE_SIDE)?);
    let (width, height) = (
        device.width.0.checked_sub(left)?.min(TILE_SIDE),
        device.height.0.checked_sub(top)?.min(TILE_SIDE),
    );
    (width > 0 && height > 0).then_some(TileRect {
        x: left,
        y: top,
        width,
        height,
    })
}

/// How many columns and rows of tiles cover a page `device` pixels across.
pub fn tile_grid(device: PixelSize) -> (u32, u32) {
    (
        device.width.0.div_ceil(TILE_SIDE),
        device.height.0.div_ceil(TILE_SIDE),
    )
}

/// The most pixels a page can be across at `zoom`: one more than the truncated size, because the
/// rasterizer truncates a float product the page's size in thousandths of a point only
/// approximates. The scheduler plans from this, so it never leaves out a tile; the render says
/// which of its tiles are past the real edge.
pub fn device_bound(size: PageSize, zoom: ZoomBucket) -> PixelSize {
    let scale = zoom.scale();
    PixelSize {
        width: PixelLen(pixels(size.width, scale).saturating_add(1)),
        height: PixelLen(pixels(size.height, scale).saturating_add(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: PixelSize = PixelSize {
        width: PixelLen(1100),
        height: PixelLen(700),
    };

    #[test]
    fn edge_tiles_are_cut_to_the_page() {
        type Row = (&'static str, u32, u32, Option<(u32, u32, u32, u32)>);
        const CASES: &[Row] = &[
            // name, column, row, (x, y, width, height)
            ("first", 0, 0, Some((0, 0, 512, 512))),
            ("right edge", 2, 0, Some((1024, 0, 76, 512))),
            ("bottom edge", 0, 1, Some((0, 512, 512, 188))),
            ("corner", 2, 1, Some((1024, 512, 76, 188))),
            ("past the right", 3, 0, None),
            ("past the bottom", 0, 2, None),
            ("overflowing", u32::MAX, 0, None),
        ];
        for (name, x, y, want) in CASES {
            let got = tile_rect(PAGE, *x, *y).map(|r| (r.x, r.y, r.width, r.height));
            assert_eq!(got, *want, "{name}");
        }
    }

    #[test]
    fn the_grid_covers_the_page() {
        const CASES: &[(&str, u32, u32, (u32, u32))] = &[
            ("exact", 1024, 512, (2, 1)),
            ("one over", 1025, 513, (3, 2)),
            ("tiny", 1, 1, (1, 1)),
        ];
        for (name, w, h, want) in CASES {
            let device = PixelSize {
                width: PixelLen(*w),
                height: PixelLen(*h),
            };
            assert_eq!(tile_grid(device), *want, "{name}");
        }
    }
}
