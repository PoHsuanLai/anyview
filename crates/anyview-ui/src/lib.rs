//! The viewer's pure state machines. Every region of the viewer (chrome, panel, palette, sheet,
//! navigation, presentation, loading, the four stages) is a [`ds_core::machine::Machine`]: it
//! takes an input and the time it happened, and returns its next state and the effects it wants
//! as data. Nothing here reads a clock, touches a file or draws; the views and the platform edge
//! above carry the effects out.
//!
//! Every public item is reached from this root, once.

mod chrome;
mod command;
mod keys;
mod load;
mod navigate;
mod palette;
mod panel;
mod presentation;
mod sheet;
mod stage;
mod time;
mod typed;
mod viewer;

#[cfg(test)]
mod testing;

pub use chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams, PinReason, PinReasons, Zone};
pub use command::{Command, StageCommand};
pub use keys::{Regions, Route, route};
pub use load::{Load, LoadFailure, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket};
pub use navigate::{Navigate, NavigateIn, NavigateOut};
pub use palette::{Palette, PaletteIn, PaletteMove, PaletteOut, PaletteParams, RowIndex};
pub use panel::{Panel, PanelIn, PanelOut, PanelParams, PanelTab, PanelTabs};
pub use presentation::{
    ContentClass, Presentation, PresentationIn, PresentationOut, PresentationParams,
};
pub use sheet::{ExportDraft, ExportFamily, ExportKindPick, Sheet, SheetIn, SheetOut};
pub use stage::{
    AfterScrub, Animation, Destination, EndReason, FindHits, FindOut, FrameCount, FrameDirection,
    FrameIndex, HitCount, HitCursor, HitIndex, HitStep, MediaError, MediaIn, MediaOut, MediaParams,
    MediaStage, Pace, PageView, PdfIn, PdfOut, PdfParams, PdfStage, PlayerCommand, PlayerEvent,
    RasterIn, RasterOut, RasterParams, RasterStage, Spin, Stage, StageFamily, StageIn, StageOut,
    StageParams, TextIn, TextOut, TextParams, TextPlace, TextStage, TextView, TextViews, TrackKind,
    Viewport, Wrap, ZoomDir,
};
pub use typed::TypedText;
pub use viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
