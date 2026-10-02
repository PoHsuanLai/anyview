//! The viewer's full tier: one view per family of formats, the trait they implement, and the
//! registry that maps every kind of file to one of them.

mod peek_only;
mod raster;
mod registry;
mod text;
mod view;

pub use peek_only::{PeekOnlyDoc, PeekOnlyStageView};
pub use raster::{RasterBackend, RasterDoc, RasterDone, RasterJob, RasterStageView, RasterTarget};
pub(crate) use registry::open_for;
pub use registry::{KindVisitor, family_of, visit};
pub(crate) use text::views_of;
pub use text::{LineWindow, TextDoc, TextStageView};
pub use view::{Area, FrameLook, Held, LoadedDoc, StageCx, StageView};

/// The colour of each token class of highlighted code, in tokens only.
pub const TOKEN_CSS: &str = include_str!("text/tokens.css");
