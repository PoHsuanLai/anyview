//! Tiles: the pure half of drawing a document. Which tiles a view needs is arithmetic over page
//! sizes, the viewport and the zoom; drawing them is `render`'s job.

mod key;
mod schedule;
mod zoom;

pub use key::{TILE_SIDE, TileKey, TileRect, device_bound, tile_grid, tile_rect};
pub use schedule::{Priority, Schedule, TileBatch, TileCoord, ViewWindow, schedule};
pub use zoom::ZoomBucket;

#[cfg(test)]
mod tests;
