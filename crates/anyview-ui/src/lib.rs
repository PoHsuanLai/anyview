//! The viewer: its pure state machines, and the views that draw them.
//!
//! Every region of the viewer (chrome, panel, palette, sheet, navigation, presentation, loading,
//! the four stages) is a [`ds_core::machine::Machine`]: it takes an input and the time it
//! happened, and returns its next state and the effects it wants as data. Those modules read no
//! clock, touch no file and draw nothing (`scripts/check-boundary.sh` holds them to it). The
//! effects are carried out by `io` (blocking work for a pool the binary owns, never a thread of
//! this library), and what the machines say is drawn by `families` and `views`.
//!
//! Every public item is reached from this root, once, except that [`prelude`] names the front doors
//! a second time for a glob import.

mod chrome;
mod command;
mod context;
mod edits;
mod families;
mod hand;
mod io;
mod keys;
mod load;
mod look;
mod navigate;
mod palette;
mod panel;
mod picture;
pub mod prelude;
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
pub use command::{Command, PictureCommand, StageCommand};
pub use context::{
    ContextEntry, ContextIn, ContextMenu, ContextOut, ContextParams, ContextPick, Spot,
};
pub use edits::{EditCaution, EditOffer, EditRequest, Rewind};
pub use families::{
    Area, FamilyVisitor, Finish, FlightId, FoundHits, FrameLook, Held, HitLine, Leaving,
    LineWindow, LoadedDoc, MEDIA_CSS, MediaDoc, MediaLive, MediaPlace, MediaShelf, MediaStageView,
    PDF_CSS, PdfAnswer, PdfAsk, PdfDoc, PdfFailure, PdfOrigin, PdfShelf, PdfStageView, PdfTask,
    PeekOnlyDoc, PeekOnlyStageView, RasterBackend, RasterDoc, RasterDone, RasterJob, RasterOpen,
    RasterStageView, ReadyTile, SheetDoc, StageCx, StageView, TOKEN_CSS, TableDoc, TableStageView,
    TextDoc, TextStageView, TreeDoc, TreeStageView, TrimMarks, audio_window_size, family_of,
    flow_of, use_media_shelf, use_pdf_shelf, visit,
};
pub use hand::{Hand, HandIn, Space, Tool};
pub use io::{
    Backend, DesktopService, Done, Edge, FileAccess, FileLocks, HelperSource, HelperWords,
    HostRequest, ImagePlugins, Job, MediaHost, MediaLine, MediaNotice, MediaPlayback, MediaStart,
    MediaStarted, MediaWake, NaturalSize, Need, Notice, OpenError, OpenPort, Opened,
    PlatformAbilities, PluginPicture, Preloaded, Readable, Reply, ResumeSource, SaveEnd, Services,
    SizeBasis, SlotPixels, Stop, TextSave, VersionSource, Work, WorkKind, WorkLane, Workers,
    folder_sequence,
};
pub use keys::{Regions, Route, route};
pub use load::{
    Freshness, Load, LoadFailure, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket, freshness,
};
pub use look::{Look, LookFeed};
pub use navigate::{Navigate, NavigateIn, NavigateOut};
pub use palette::{
    HitList, Palette, PaletteIn, PaletteIndex, PaletteMove, PaletteOut, PaletteParams, PaletteScope,
};
pub use panel::{Panel, PanelIn, PanelOut, PanelParams, PanelTab, PanelTabs};
pub use picture::{
    CropAspect, CropBox, CropGrip, CropLean, CropShape, PictureEditIn, PictureEditing, PictureEdits,
};
pub use presentation::{
    ContentClass, Presentation, PresentationIn, PresentationOut, PresentationParams,
};
pub use sheet::{
    Departure, ExportControl, ExportDraft, ExportFacts, ExportFamily, ExportKindPick, ExportOption,
    HelperEnd, HelperPhase, MAX_LONG_EDGE, MAX_RESIZED_SIDE, MediaOffer, PageSpan, PictureSheet,
    PictureSheetIn, PictureSheetOut, ResizeChange, ResizeDraft, ResizeProportion, ResizeUnit,
    Sheet, SheetIn, SheetOut, SheetParams, SizePick, TrimSpan, VersionKey, VersionList, VersionRow,
    format_of, kind_hint, kind_name, quality_of,
};
pub use stage::{
    AfterScrub, Animation, Changes, ControlOffer, Destination, EditFind, Editable, Edited,
    EndReason, FindHits, FindOut, FrameCount, FrameDelays, FrameIndex, HitCount, HitCursor,
    HitIndex, HitStep, LineTotal, MediaAbilities, MediaError, MediaIn, MediaOut, MediaParams,
    MediaStage, Motion, Outside, Pace, PageEdits, PageLines, PageView, PdfIn, PdfOut, PdfParams,
    PdfStage, PlayerCommand, PlayerEvent, Playing, RasterIn, RasterOut, RasterParams, RasterStage,
    RowNo, RowStep, Runs, SheetNo, SheetTotal, Stage, StageAbilities, StageFamily, StageIn,
    StageOut, StageParams, StepDirection, TableIn, TableOut, TableParams, TableStage, TextExtent,
    TextIn, TextOut, TextParams, TextPlace, TextStage, TextStep, TextView, TextViews, TrackKind,
    TreeIn, TreeOut, TreeParams, TreeStage, TrimEdge, Viewport, Wrap, ZoomDir,
};
pub use typed::TypedText;
pub use viewer::{Choosing, PanelSay, Trashing, Viewer, ViewerIn, ViewerOut, ViewerParams};
pub use views::{Launch, ViewerApp, WelcomeApp, stylesheet};
