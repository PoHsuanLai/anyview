//! The viewer's pure state machines, with nothing to draw them.
//!
//! Every region of the viewer (chrome, panel, palette, sheet, navigation, presentation, loading,
//! the stages, the root) is a [`ds_core::machine::Machine`]: it takes an input and the time it
//! happened, and returns its next state and the effects it wants as data. These modules read no
//! clock, touch no file, spawn no thread and draw nothing (`scripts/check-boundary.sh` holds the
//! crate to it, and to having no Dioxus, no `ds` and no window in its dependency tree), so a host
//! that is not the viewer's window (a terminal, another app's pane) can drive the same machines.
//! `anyview-ui` carries the effects out and draws what they say; it re-exports everything here, so
//! its callers name `anyview_ui::Viewer` as ever.
//!
//! Every public item is reached from this root, once. [`seam`] is the exception: a few names the
//! views need that are not part of the machines' surface.

#![warn(missing_docs)]

mod abilities;
mod chrome;
mod command;
mod context;
mod edits;
mod hand;
mod keys;
mod load;
mod markup;
mod navigate;
mod palette;
mod panel;
mod picture;
mod presentation;
mod remembering;
mod sheet;
mod stage;
#[cfg(test)]
mod testing;
mod time;
mod trail;
mod trim;
mod typed;
mod viewer;

pub use abilities::{DesktopService, PlatformAbilities};
pub use chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams, PinReason, PinReasons, Zone};
pub use command::{Command, MarkupCommand, PictureCommand, StageCommand};
pub use context::{
    ContextEntry, ContextIn, ContextMenu, ContextOut, ContextParams, ContextPick, Spot,
};
pub use edits::{EditCaution, EditOffer, EditRequest, Rewind, SaveEnd};
pub use hand::{Hand, HandIn, Space, Tool};
pub use keys::{Act, Chords, Press, Regions, Route, route};
pub use load::{
    Freshness, Load, LoadFailure, LoadFlow, LoadIn, LoadOut, PeekFrame, Ticket, freshness,
};
pub use markup::{Allowance, Draft, MarkupIn, MarkupOut, PdfMarkup, Using};
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
pub use remembering::{Noted, REMEMBER_EVERY, Remembering};
pub use sheet::{
    Departure, ExportControl, ExportDraft, ExportFacts, ExportFamily, ExportKindPick, ExportOption,
    HelperEnd, HelperPhase, MAX_LONG_EDGE, MAX_RESIZED_SIDE, MediaOffer, PageSpan, PictureSheet,
    PictureSheetIn, PictureSheetOut, ResizeChange, ResizeDraft, ResizeProportion, ResizeUnit,
    Sheet, SheetIn, SheetOut, SheetParams, SizePick, TrimSpan, VersionKey, VersionList, VersionRow,
    Wanted, format_of, kind_hint, kind_name, quality_of,
};
pub use stage::{
    AfterScrub, AfterTyping, Animation, CellFocus, CellStep, Cells, Changes, ColumnNo, ColumnSort,
    ContentScale, ControlOffer, DataEdits, Destination, EditDepth, EditLog, EditRevision,
    EditSessionIn, EditSessionOut, Editable, Edited, EndReason, FindHits, FindOut, FrameCount,
    FrameDelays, FrameIndex, HitCount, HitCursor, HitIndex, HitStep, KeyUnder, LineTotal,
    MediaAbilities, MediaError, MediaIn, MediaOut, MediaParams, MediaStage, Motion, NodeUnder,
    Outside, Pace, PageEdits, PageLines, PageView, PdfIn, PdfOut, PdfParams, PdfStage,
    PlayerCommand, PlayerEvent, Playing, RasterIn, RasterOut, RasterParams, RasterStage, RowNo,
    RowOrder, RowStep, Runs, SheetNo, SheetTotal, SortDirection, Stage, StageAbilities,
    StageFamily, StageFind, StageIn, StageOut, StageParams, StepDirection, TableIn, TableOut,
    TableParams, TableStage, TextExtent, TextIn, TextOut, TextParams, TextPlace, TextStage,
    TextStep, TextView, TextViews, TrackKind, TreeIn, TreeOut, TreeParams, TreeStage, TrimEdge,
    TypingIn, TypingSeed, TypingState, Viewport, Wrap, Writing, ZoomDir, is_numeric, sorted_rows,
};
pub use stage::{OutlineFold, OutlineIn, OutlineOut, OutlineShape};
pub use trail::{Trail, TrailIn, TrailOut, TrailStacks};
pub use trim::{MIN_KEPT, MediaTrim, TrimIn, TrimWindow};
pub use typed::TypedText;
pub use viewer::{Choosing, PanelSay, Trashing, Viewer, ViewerIn, ViewerOut, ViewerParams};

/// What `anyview-ui`'s views need of the machines beyond their surface: the names its key
/// registration and its context menu and wrap choices are built from. Not part of the API.
#[doc(hidden)]
pub mod seam {
    pub use crate::context::entries;
    pub use crate::keys::{app, rows, standing};
    pub use crate::stage::WrapChoices;
}
