//! The viewer's full tier: one view per family of formats, the trait they implement, and the
//! registry that maps every kind of file to one of them.

mod find_bar;
mod pdf;
mod peek_only;
mod raster;
mod registry;
mod text;
mod view;

pub use pdf::{
    Finish, FlightId, PdfAnswer, PdfAsk, PdfDoc, PdfFailure, PdfShelf, PdfStageView, PdfTask,
    ReadyTile, use_pdf_shelf,
};
pub use peek_only::{PeekOnlyDoc, PeekOnlyStageView};
pub use raster::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterStageView, RasterTarget};
pub use registry::{KindVisitor, family_of, flow_of, visit};
pub(crate) use registry::{open_for, peek_for};
pub(crate) use text::top_for;
pub(crate) use text::views_of;
pub use text::{FoundHits, LineWindow, TextDoc, TextStageView};
pub use view::{Area, FrameLook, Held, LoadedDoc, StageCx, StageView};

/// The colour of each token class of highlighted code, in tokens only.
pub const TOKEN_CSS: &str = include_str!("text/tokens.css");

/// The PDF stage's stylesheet: the pages, the marks over them, the find bar, the panel's lists.
pub const PDF_CSS: &str = include_str!("pdf/pdf.css");
