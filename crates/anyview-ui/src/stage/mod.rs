//! The stages: what shows a file's content, one machine per family of formats.

mod dispatch;
mod family;
mod find;
mod media;
mod model;
mod pdf;
mod raster;
mod resume;
mod step;
#[cfg(test)]
mod tests;
mod text;
mod zoom;

pub use family::StageFamily;
pub use find::{FindHits, FindOut, HitCount, HitCursor, HitIndex, HitStep};
pub use media::{
    AfterScrub, EndReason, FrameDirection, MediaError, MediaIn, MediaOut, MediaParams, MediaStage,
    Pace, PlayerCommand, PlayerEvent, TrackKind,
};
pub use model::{Stage, StageIn, StageOut, StageParams};
pub use pdf::{Destination, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub use raster::{
    Animation, FrameCount, FrameIndex, RasterIn, RasterOut, RasterParams, RasterStage, Spin,
};
pub use text::{
    LineTotal, PageLines, TextExtent, TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep,
    TextView, TextViews, Wrap,
};
pub use zoom::{Viewport, ZoomDir};
