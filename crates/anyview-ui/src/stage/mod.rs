//! The stages: what shows a file's content, one machine per family of formats.

mod family;
mod find;
mod raster;
mod zoom;

pub use family::StageFamily;
pub use find::{FindHits, FindOut, HitCount, HitCursor, HitIndex, HitStep};
pub use raster::{
    Animation, FrameCount, FrameIndex, RasterIn, RasterOut, RasterParams, RasterStage, Spin,
};
pub use zoom::{Viewport, ZoomDir};
