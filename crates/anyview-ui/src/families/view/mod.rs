//! The full tier of one family of formats: what it opens, and how it draws, finds its controls
//! and fills the side panel. [`StageView`] is the trait each family implements once (the plan's
//! `Stage`, named for what it draws: the viewer's `Stage` is the machine enum of the stage
//! region); [`DocView`](doc::DocView) is the loaded document seen without its family, which is
//! what the window holds, so the window needs no match on the family to draw it.

mod cx;
mod doc;
mod shared;
mod stage;

pub(crate) use cx::WHEEL_ZOOM;
pub use cx::{Area, FrameLook, HitLine, Leaving, StageCx};
pub use doc::LoadedDoc;
pub use shared::{EditDone, Held, SaveBytes};
pub use stage::StageView;
