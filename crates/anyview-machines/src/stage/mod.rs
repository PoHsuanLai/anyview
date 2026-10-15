//! The stages: what shows a file's content, one machine per family of formats.

mod abilities;
mod data_edits;
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

pub use abilities::{Playing, StageAbilities, Writing};
pub use data_edits::{
    AfterTyping, DataEdits, EditDepth, EditLog, EditRevision, EditSessionIn, EditSessionOut,
    RowOrder, TypingIn, TypingSeed, TypingState,
};
pub use family::StageFamily;
pub use find::{FindHits, FindOut, HitCount, HitCursor, HitIndex, HitStep, StageFind};
pub use media::{
    AfterScrub, ControlOffer, EndReason, MediaAbilities, MediaError, MediaIn, MediaOut,
    MediaParams, MediaStage, Pace, PlayerCommand, PlayerEvent, StepDirection, TrackKind, TrimEdge,
};
pub use model::{Stage, StageIn, StageOut, StageParams};
pub use pdf::{Destination, PageEdits, PageView, PdfIn, PdfOut, PdfParams, PdfStage};
pub use pdf::{OutlineFold, OutlineIn, OutlineOut, OutlineShape};
pub use raster::{
    Animation, FrameCount, FrameDelays, FrameIndex, Motion, RasterIn, RasterOut, RasterParams,
    RasterStage, Runs,
};
pub use row::{RowNo, RowStep};
pub use table::{
    CellFocus, CellStep, Cells, ColumnNo, ColumnSort, SheetNo, SheetTotal, SortDirection, TableIn,
    TableOut, TableParams, TableStage, is_numeric, sorted_rows,
};
pub use text::WrapChoices;
pub use text::{
    Changes, Editable, Edited, LineTotal, Outside, PageLines, TextExtent, TextIn, TextOut,
    TextParams, TextPlace, TextStage, TextStep, TextView, TextViews, Wrap,
};
pub use tree::{KeyUnder, NodeUnder, TreeIn, TreeOut, TreeParams, TreeStage};
pub use zoom::{ContentScale, Viewport, ZoomDir};
