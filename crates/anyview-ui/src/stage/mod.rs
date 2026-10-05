//! The stages: what shows a file's content, one machine per family of formats.

mod book;
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

pub use book::{BookIn, BookOut, BookParams, BookStage};
pub use family::StageFamily;
pub use find::{FindHits, FindOut, HitCount, HitCursor, HitIndex, HitStep};
pub use media::{
    AfterScrub, EndReason, MediaError, MediaIn, MediaOut, MediaParams, MediaStage, Pace,
    PlayerCommand, PlayerEvent, StepDirection, TrackKind, TrimEdge,
};
pub use model::{Stage, StageIn, StageOut, StageParams};
pub use pdf::{Destination, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub use raster::{
    Animation, FrameCount, FrameDelays, FrameIndex, Motion, RasterIn, RasterOut, RasterParams,
    RasterStage, Runs,
};
pub use text::{
    LineTotal, PageLines, TextExtent, TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep,
    TextView, TextViews, Wrap,
};
pub use zoom::{Viewport, ZoomDir};
