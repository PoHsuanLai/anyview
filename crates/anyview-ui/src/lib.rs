//! The viewer: its pure state machines, and the views that draw them.
//!
//! Every region of the viewer (chrome, panel, palette, sheet, navigation, presentation, loading,
//! the four stages) is a [`ds_core::machine::Machine`]: it takes an input and the time it
//! happened, and returns its next state and the effects it wants as data. Those modules read no
//! clock, touch no file and draw nothing (`scripts/check-boundary.sh` holds them to it). The
//! effects are carried out by `io` (blocking work for a pool the binary owns, never a thread of
//! this library), and what the machines say is drawn by `families` and `views`.
//!
//! Every public item is reached from this root, once.

mod chrome;
mod command;
mod families;
mod io;
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
mod views;

#[cfg(test)]
mod testing;

pub use chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams, PinReason, PinReasons, Zone};
pub use command::{Command, StageCommand};
pub use families::{
    Area, FrameLook, Held, KindVisitor, LineWindow, LoadedDoc, PeekOnlyDoc, PeekOnlyStageView,
    RasterBackend, RasterDoc, RasterDone, RasterJob, RasterStageView, RasterTarget, StageCx,
    StageView, TOKEN_CSS, TextDoc, TextStageView, family_of, visit,
};
pub use io::{
    Backend, Done, Edge, HostRequest, Job, OpenError, OpenLink, Probed, Reply, Stop, Work, Workers,
};
pub use keys::{Regions, Route, route};
pub use load::{
    Freshness, Load, LoadFailure, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket, freshness,
};
pub use navigate::{Navigate, NavigateIn, NavigateOut};
pub use palette::{Palette, PaletteIn, PaletteMove, PaletteOut, PaletteParams, RowIndex};
pub use panel::{Panel, PanelIn, PanelOut, PanelParams, PanelTab, PanelTabs};
pub use presentation::{
    ContentClass, Presentation, PresentationIn, PresentationOut, PresentationParams,
};
pub use sheet::{ExportDraft, ExportFamily, ExportKindPick, Sheet, SheetIn, SheetOut};
pub use stage::{
    AfterScrub, Animation, Destination, EndReason, FindHits, FindOut, FrameCount, FrameDirection,
    FrameIndex, HitCount, HitCursor, HitIndex, HitStep, LineTotal, MediaError, MediaIn, MediaOut,
    MediaParams, MediaStage, Pace, PageLines, PageView, PdfIn, PdfOut, PdfParams, PdfStage,
    PlayerCommand, PlayerEvent, RasterIn, RasterOut, RasterParams, RasterStage, Spin, Stage,
    StageFamily, StageIn, StageOut, StageParams, TextExtent, TextIn, TextOut, TextParams,
    TextPlace, TextStage, TextStep, TextView, TextViews, TrackKind, Viewport, Wrap, ZoomDir,
};
pub use typed::TypedText;
pub use viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
pub use views::{Launch, ViewerApp, stylesheet};
