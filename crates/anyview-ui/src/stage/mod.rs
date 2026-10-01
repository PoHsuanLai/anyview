//! The stages: what shows a file's content, one machine per family of formats.

mod family;
mod find;
mod media;
mod pdf;
mod raster;
mod text;
mod zoom;

pub use family::StageFamily;
pub use find::{FindHits, FindOut, HitCount, HitCursor, HitIndex, HitStep};
pub use media::{
    AfterScrub, EndReason, FrameDirection, MediaError, MediaIn, MediaOut, MediaParams, MediaStage,
    Pace, PlayerCommand, PlayerEvent, TrackKind,
};
pub use pdf::{Destination, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub use raster::{
    Animation, FrameCount, FrameIndex, RasterIn, RasterOut, RasterParams, RasterStage, Spin,
};
pub use text::{TextIn, TextOut, TextParams, TextPlace, TextStage, TextView, TextViews, Wrap};
pub use zoom::{Viewport, ZoomDir};
