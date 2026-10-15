//! The viewer: the views that draw its state machines, and the effects that feed them.
//!
//! Every region of the viewer (chrome, panel, palette, sheet, navigation, presentation, loading,
//! the stages) is a [`ds_core::machine::Machine`], and those live in `anyview-machines`, which has
//! no Dioxus and no window: they take an input and the time it happened, and return their next
//! state and the effects they want as data. This crate re-exports them, so a caller names
//! `anyview_ui::Viewer` as ever. The effects are carried out by `io` (blocking work for a pool the
//! binary owns, never a thread of this library), and what the machines say is drawn by `families`
//! and `views`.
//!
//! Every public item is reached from this root, once, except that [`prelude`] names the front doors
//! a second time for a glob import.

#![warn(missing_docs)]

mod families;
mod io;
mod look;
/// The names a host uses most, for `use anyview_ui::prelude::*;`.
pub mod prelude;
#[cfg(test)]
mod testing;
mod views;

pub use anyview_machines::{
    Act, AfterScrub, Allowance, Animation, Changes, Choosing, Chords, Chrome, ChromeIn, ChromeOut,
    ChromeParams, ColumnSort, Command, ContentClass, ContentScale, ContextEntry, ContextIn,
    ContextMenu, ContextOut, ContextParams, ContextPick, ControlOffer, CropAspect, CropBox,
    CropGrip, CropLean, CropShape, Departure, Destination, Draft, EditCaution, EditOffer,
    EditRequest, Editable, Edited, EndReason, ExportControl, ExportDraft, ExportFacts,
    ExportFamily, ExportKindPick, ExportOption, FindHits, FindOut, FrameCount, FrameDelays,
    FrameIndex, Freshness, Hand, HandIn, HelperEnd, HelperPhase, HitCount, HitCursor, HitIndex,
    HitList, HitStep, LineTotal, Load, LoadFailure, LoadFlow, LoadIn, LoadOut, MAX_LONG_EDGE,
    MAX_RESIZED_SIDE, MIN_KEPT, MarkupCommand, MarkupIn, MarkupOut, MediaAbilities, MediaError,
    MediaIn, MediaOffer, MediaOut, MediaParams, MediaStage, MediaTrim, Motion, Navigate,
    NavigateIn, NavigateOut, Noted, Outside, Pace, PageEdits, PageLines, PageSpan, PageView,
    Palette, PaletteIn, PaletteIndex, PaletteMove, PaletteOut, PaletteParams, PaletteScope, Panel,
    PanelIn, PanelOut, PanelParams, PanelSay, PanelTab, PanelTabs, PdfIn, PdfMarkup, PdfOut,
    PdfParams, PdfStage, PeekFrame, PictureCommand, PictureEditIn, PictureEditing, PictureEdits,
    PictureSheet, PictureSheetIn, PictureSheetOut, PinReason, PinReasons, PlayerCommand,
    PlayerEvent, Playing, Presentation, PresentationIn, PresentationOut, PresentationParams, Press,
    REMEMBER_EVERY, RasterIn, RasterOut, RasterParams, RasterStage, Regions, Remembering,
    ResizeChange, ResizeDraft, ResizeProportion, ResizeUnit, Rewind, Route, RowNo, RowStep, Runs,
    Sheet, SheetIn, SheetNo, SheetOut, SheetParams, SheetTotal, SizePick, SortDirection, Space,
    Spot, Stage, StageAbilities, StageCommand, StageFamily, StageFind, StageIn, StageOut,
    StageParams, StepDirection, TableIn, TableOut, TableParams, TableStage, TextExtent, TextIn,
    TextOut, TextParams, TextPlace, TextStage, TextStep, TextView, TextViews, Ticket, Tool,
    TrackKind, Trashing, TreeIn, TreeOut, TreeParams, TreeStage, TrimEdge, TrimIn, TrimSpan,
    TrimWindow, TypedText, Using, VersionKey, VersionList, VersionRow, Viewer, ViewerIn, ViewerOut,
    ViewerParams, Viewport, Wanted, Wrap, Writing, Zone, ZoomDir, format_of, freshness, is_numeric,
    kind_hint, kind_name, quality_of, route, sorted_rows,
};
pub use families::{
    Area, FamilyVisitor, Finish, FlightId, FoundHits, FrameLook, Held, HitLine, Leaving,
    LineWindow, LoadedDoc, MEDIA_CSS, MediaDoc, MediaLive, MediaPlace, MediaShelf, MediaStageView,
    PDF_CSS, PdfAnswer, PdfAsk, PdfDoc, PdfFailure, PdfOrigin, PdfShelf, PdfStageView, PdfTask,
    PeekOnlyDoc, PeekOnlyStageView, RasterBackend, RasterDoc, RasterDone, RasterJob, RasterOpen,
    RasterStageView, ReadyTile, SheetDoc, SortedOrder, StageCx, StageView, TOKEN_CSS, TableDoc,
    TableStageView, TextDoc, TextStageView, TreeDoc, TreeStageView, TrimMarks, audio_window_size,
    family_of, flow_of, use_media_shelf, use_pdf_shelf, visit,
};
pub use io::{
    Backend, DesktopService, Done, Edge, FileAccess, FileLocks, HelperSource, HelperWords,
    HostRequest, ImagePlugins, Job, Keeping, MediaHost, MediaLine, MediaNotice, MediaPlayback,
    MediaStart, MediaStarted, MediaSupport, MediaWake, NaturalSize, Need, NotOpened, Notice,
    OpenError, OpenPort, Opened, PlatformAbilities, PluginPicture, Preloaded, Readable, Reply,
    ResumeKeeper, ResumeSource, SaveEnd, Seams, SizeBasis, SlotPixels, Stop, TextSave,
    VersionSource, Work, WorkKind, WorkLane, Workers, folder_sequence,
};
pub use look::{Look, LookFeed, LookSender};
pub use views::{
    Launch, PaneApp, PaneChrome, PaneLink, PaneSeat, ViewerApp, WelcomeApp, stylesheet,
    use_pane_link,
};
