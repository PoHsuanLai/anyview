//! The viewer's full tier: one view per family of formats, the trait they implement, and the
//! registry that maps every kind of file to one of them.

mod card;
mod found;
mod media;
mod pdf;
mod peek_only;
mod raster;
mod registry;
mod rows;
mod table;
mod text;
mod tree;
mod view;

pub(crate) use card::InfoCard;
pub use media::{
    MediaDoc, MediaLive, MediaPlace, MediaShelf, MediaStageView, TrimMarks, audio_window_size,
    use_media_shelf,
};
pub(crate) use media::{level_to_volume, place_to_time, tabs_offered as media_tabs};
pub use pdf::{
    Finish, FlightId, PdfAnswer, PdfAsk, PdfDoc, PdfFailure, PdfShelf, PdfStageView, PdfTask,
    ReadyTile, use_pdf_shelf,
};
pub use peek_only::{PeekOnlyDoc, PeekOnlyStageView};
pub use raster::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterStageView, RasterTarget};
pub use registry::{KindVisitor, family_of, flow_of, visit};
pub(crate) use registry::{open_for, peek_for};
pub use table::{SheetDoc, TableDoc, TableStageView};
pub(crate) use text::top_for;
pub(crate) use text::views_of;
pub use text::{FoundHits, LineWindow, TextDoc, TextStageView};
pub use tree::{TreeDoc, TreeStageView};
pub use view::{Area, FrameLook, Held, HitLine, Leaving, LoadedDoc, StageCx, StageView};

/// The colour of each token class of highlighted code, in tokens only.
pub const TOKEN_CSS: &str = include_str!("text/tokens.css");

/// The media stage's stylesheet: the picture, the album card, the panel's tracks and chapters.
pub const MEDIA_CSS: &str = include_str!("media/media.css");

/// The PDF stage's stylesheet: the pages, the marks over them, the panel's lists.
pub const PDF_CSS: &str = include_str!("pdf/pdf.css");

/// The file card's stylesheet.
pub(crate) const CARD_CSS: &str = include_str!("card.css");

/// The picture stage's stylesheet.
pub(crate) const RASTER_CSS: &str = include_str!("raster/raster.css");

/// The text stage's stylesheet: the source lines and the marks of a find.
pub(crate) const TEXT_CSS: &str = include_str!("text/source.css");

/// The table and tree stages' stylesheet.
pub(crate) const DATA_CSS: &str = include_str!("table/data.css");
