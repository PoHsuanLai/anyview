//! The raster stage: an image fitted, zoomed or being dragged, turned, and perhaps animated.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{
    Animation, FrameCount, FrameDelays, FrameIndex, Motion, RasterIn, RasterOut, RasterParams,
    RasterStage, Runs,
};
