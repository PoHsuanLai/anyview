//! The stages: what shows a file's content, one machine per family of formats.

mod abilities;
mod dispatch;
mod family;
mod find;
mod media;
mod model;
mod pdf;
mod raster;
mod resume;
mod row;
mod step;
mod table;
#[cfg(test)]
mod tests;
mod text;
mod tree;
mod zoom;

pub use abilities::StageAbilities;
pub use family::StageFamily;
pub use find::{FindHits, FindOut, HitCount, HitCursor, HitIndex, HitStep};
pub use media::{
    AfterScrub, ControlOffer, EndReason, MediaAbilities, MediaError, MediaIn, MediaOut,
    MediaParams, MediaStage, Pace, PlayerCommand, PlayerEvent, StepDirection, TrackKind, TrimEdge,
};
pub use model::{Stage, StageIn, StageOut, StageParams};
pub use pdf::{Destination, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub use raster::{
    Animation, FrameCount, FrameDelays, FrameIndex, Motion, RasterIn, RasterOut, RasterParams,
    RasterStage, Runs,
};
pub use row::{RowNo, RowStep};
pub use table::{SheetNo, SheetTotal, TableIn, TableOut, TableParams, TableStage};
pub use text::{
    LineTotal, PageLines, TextExtent, TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep,
    TextView, TextViews, Wrap,
};
pub use tree::{TreeIn, TreeOut, TreeParams, TreeStage};
pub use zoom::{Viewport, ZoomDir};
