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
mod context;
mod edits;
mod families;
mod io;
mod keys;
mod load;
mod look;
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
pub use context::{
    ContextEntry, ContextIn, ContextMenu, ContextOut, ContextParams, ContextPick, Spot,
};
pub use edits::{EditCaution, EditOffer, EditRequest, Rewind};
pub use families::{
    Area, BookDoc, BookStageView, Finish, FlightId, FoundHits, FrameLook, Held, KindVisitor,
    Layout, Leaving, LineWindow, LoadedDoc, MEDIA_CSS, MediaDoc, MediaLive, MediaPlace, MediaShelf,
    MediaStageView, PDF_CSS, PdfAnswer, PdfAsk, PdfDoc, PdfFailure, PdfShelf, PdfStageView,
    PdfTask, PeekOnlyDoc, PeekOnlyStageView, RasterBackend, RasterDoc, RasterDone, RasterJob,
    RasterStageView, RasterTarget, ReadyTile, SectionPage, SheetDoc, StageCx, StageView, TOKEN_CSS,
    TableDoc, TableStageView, TextDoc, TextStageView, TreeDoc, TreeStageView, TrimMarks, family_of,
    flow_of, use_media_shelf, use_pdf_shelf, visit,
};
pub use io::{
    Backend, Done, Edge, FileAccess, FileCard, FileCards, FileLocks, FirstFrameSource, HostRequest,
    ImagePlugins, Job, MediaHost, MediaLine, MediaNotice, MediaPlayback, MediaStart, MediaStarted,
    MediaWake, Notice, OpenError, OpenLink, PluginPicture, Preloaded, Probed, Readable, Reply,
    ResumeSource, SlotPixels, Stop, VersionSource, Work, WorkKind, WorkLane, Workers,
    folder_sequence,
};
pub use keys::{Regions, Route, route};
pub use load::{
    Freshness, Load, LoadFailure, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket, freshness,
};
pub use look::{Look, LookFeed};
pub use navigate::{Navigate, NavigateIn, NavigateOut};
pub use palette::{Palette, PaletteIn, PaletteMove, PaletteOut, PaletteParams, RowIndex};
pub use panel::{Panel, PanelIn, PanelOut, PanelParams, PanelTab, PanelTabs};
pub use presentation::{
    ContentClass, Presentation, PresentationIn, PresentationOut, PresentationParams,
};
pub use sheet::{
    ExportDraft, ExportFamily, ExportKindPick, MediaOffer, Sheet, SheetIn, SheetOut, SheetParams,
    VersionKey, VersionList, VersionRow,
};
pub use stage::{
    AfterScrub, Animation, BookIn, BookOut, BookParams, BookStage, ControlOffer, Destination,
    EndReason, FindHits, FindOut, FrameCount, FrameDelays, FrameIndex, HitCount, HitCursor,
    HitIndex, HitStep, LineTotal, MediaAbilities, MediaError, MediaIn, MediaOut, MediaParams,
    MediaStage, Motion, Pace, PageLines, PageView, PdfIn, PdfOut, PdfParams, PdfStage,
    PlayerCommand, PlayerEvent, RasterIn, RasterOut, RasterParams, RasterStage, RowNo, RowStep,
    Runs, SheetNo, SheetTotal, Stage, StageFamily, StageIn, StageOut, StageParams, StepDirection,
    TableIn, TableOut, TableParams, TableStage, TextExtent, TextIn, TextOut, TextParams, TextPlace,
    TextStage, TextStep, TextView, TextViews, TrackKind, TreeIn, TreeOut, TreeParams, TreeStage,
    TrimEdge, Viewport, Wrap, ZoomDir,
};
pub use typed::TypedText;
pub use viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
pub use views::{Launch, ViewerApp, WelcomeApp, stylesheet};
