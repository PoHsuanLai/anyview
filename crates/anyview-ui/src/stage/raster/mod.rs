//! The raster stage: an image fitted, zoomed or being dragged, turned, and perhaps animated.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{
    Animation, FrameCount, FrameIndex, RasterIn, RasterOut, RasterParams, RasterStage,
};
