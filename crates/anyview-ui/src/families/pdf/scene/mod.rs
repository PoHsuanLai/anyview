//! Where the pages are in the room: the stack of pages at the scale on screen, what scale each zoom
//! means for this window, which part of the stack the room shows and what that asks of the tile
//! scheduler. Pure arithmetic over integers: a float appears only where a length becomes CSS.
//!
//! Every length here is a device pixel and every scale is device pixels per point, so `Actual`
//! (1000) draws a point on one device pixel, as the raster stage draws a texel.

mod frame;
mod points;
mod stack;

pub(super) use frame::{Frame, fits, pinched, scale_of};
pub(super) use stack::{Scene, distance, drawn, percent};
