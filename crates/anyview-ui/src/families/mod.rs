//! The viewer's full tier: one view per family of formats, the trait they implement, and the
//! registry that maps every kind of file to one of them.

mod card;
mod found;
mod media;
mod pdf;
mod peek_only;
mod places;
mod raster;
mod registry;
mod rows;
mod specimen;
mod table;
mod text;
mod tree;
mod unavailable;
mod view;

use anyview_core::{FactValue, FileName, Sniffed, Source, kind_name_of};

pub(crate) use card::InfoCard;
pub use media::{
    MediaDoc, MediaLive, MediaPlace, MediaShelf, MediaStageView, TrimMarks, audio_window_size,
    use_media_shelf,
};
pub(crate) use media::{bar_time, level_to_volume, tabs_offered as media_tabs};
pub use pdf::{
    Finish, FlightId, PdfAnswer, PdfAsk, PdfDoc, PdfFailure, PdfOrigin, PdfShelf, PdfStageView,
    PdfTask, ReadyTile, use_pdf_shelf,
};
pub use peek_only::{PeekOnlyDoc, PeekOnlyStageView};
pub(crate) use raster::adjusted_raster;
pub use raster::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterOpen, RasterStageView};
pub use registry::{FamilyVisitor, family_of, flow_of, visit};
pub(crate) use registry::{open_for, peek_for};
pub use table::{SheetDoc, SortedOrder, TableDoc, TableStageView};
pub(crate) use table::{search_sheet, sort_sheet};
pub(crate) use text::found_in;
pub(crate) use text::top_for;
pub(crate) use text::views_of;
pub use text::{FoundHits, LineWindow, TextDoc, TextStageView};
pub(crate) use text::{Mark, Snippet, clip, cut};
pub(crate) use tree::search_tree;
pub use tree::{TreeDoc, TreeStageView};
pub(crate) use unavailable::{UnavailableCard, listed, note_of};
pub use view::{Area, FrameLook, Held, HitLine, Leaving, LoadedDoc, StageCx, StageView};

/// The Kind row of the file `src`: what `sniffed` is in a person's words (`AVI video`, `BIN file`),
/// never its media type.
pub(crate) fn kind_row(src: &Source, sniffed: &Sniffed) -> FactValue {
    let name = src.path().file_name().unwrap_or_else(FileName::unnamed);
    FactValue::text(kind_name_of(sniffed, &name))
}

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
