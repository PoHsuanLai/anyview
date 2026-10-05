//! The root's state, inputs, outputs and parameters.

use crate::chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams};
use crate::command::Command;
use crate::edits::{EditRequest, Rewind};
use crate::load::{Load, LoadIn, LoadOut, Ticket};
use crate::navigate::{Navigate, NavigateIn};
use crate::palette::{Palette, PaletteIn, PaletteOut, PaletteParams};
use crate::panel::{Panel, PanelIn, PanelOut, PanelParams};
use crate::presentation::{Presentation, PresentationIn, PresentationOut, PresentationParams};
use crate::sheet::{Sheet, SheetIn, SheetOut, SheetParams};
use crate::stage::{Stage, StageIn, StageOut, StageParams};
use anyview_core::{FileAction, FilePath};
use ds_core::vocab::Shortcut;

/// One window's viewer: a state per region. Regions are independent machines; what couples them
/// is in `step`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Viewer {
    /// Opening the file.
    pub load: Load,
    /// The hover chrome.
    pub chrome: Chrome,
    /// The side panel.
    pub panel: Panel,
    /// The ⌘K palette.
    pub palette: Palette,
    /// The modal sheet.
    pub sheet: Sheet,
    /// Walking the sequence.
    pub navigate: Navigate,
    /// How the viewer is on screen.
    pub presentation: Presentation,
    /// What shows the content.
    pub stage: Stage,
}

impl Viewer {
    /// A viewer launched in `presentation` (a quick look, a window, a background session).
    pub fn launched(presentation: Presentation) -> Viewer {
        Viewer {
            presentation,
            ..Viewer::default()
        }
    }
}

/// What moves the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerIn {
    /// Open this file, leaving whatever was open. A load result for the file left behind is
    /// ignored by its ticket.
    Open(FilePath),
    /// Open this file again because it changed on disk, keeping where the person is in it: the
    /// stage stays while the new copy loads, and what shows stays until it lands.
    Reload(FilePath),
    /// Files were dropped on the window: the first one opens. A single file's folder becomes the
    /// sequence (the window lists it and answers with `Navigate(Start)`); several files are the
    /// sequence themselves.
    Dropped(Vec<FilePath>),
    /// A result of the load in flight. `LoadIn::Begin` is not sent this way; `Open` begins one.
    Load(LoadIn),
    /// The hover chrome.
    Chrome(ChromeIn),
    /// The side panel.
    Panel(PanelIn),
    /// The palette.
    Palette(PaletteIn),
    /// The sheet.
    Sheet(SheetIn),
    /// Walking the sequence.
    Navigate(NavigateIn),
    /// How the viewer is on screen.
    Presentation(PresentationIn),
    /// The window was opened in this presentation: it is so from the start, and nothing is
    /// asked of the host (it made the window that way).
    StartAs(Presentation),
    /// The stage showing the file.
    Stage(StageIn),
    /// Run a command from a control the window drew (a capsule button): the same thing the
    /// palette runs for the row it picked.
    Run(Command),
    /// A key press, routed by `route`.
    Key(Shortcut),
    /// The time `wake()` named has come.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for ViewerIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        ViewerIn::Elapsed
    }
}

/// What the viewer wants done: its regions' outputs, lifted, and what only the root decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerOut {
    /// Sniff this file for this load.
    Probe { ticket: Ticket, path: FilePath },
    /// Sniff this file again for this load, leaving what shows in place until the result lands.
    Reload { ticket: Ticket, path: FilePath },
    /// List the files beside this one for the sequence: the answer is `Navigate(Start)`, or
    /// nothing when the folder cannot be read.
    ListFolder(FilePath),
    /// From the load, except its probe (above).
    Load(LoadOut),
    /// From the chrome.
    Chrome(ChromeOut),
    /// From the panel.
    Panel(PanelOut),
    /// From the palette, except what it ran (it is dispatched, not forwarded).
    Palette(PaletteOut),
    /// From the sheet.
    Sheet(SheetOut),
    /// Load the neighbours of the open file; the file the walk moved onto is opened by `Probe`.
    Preload(anyview_core::Neighbours),
    /// From the presentation.
    Presentation(PresentationOut),
    /// From the stage.
    Stage(StageOut),
    /// Do this to the open file: an action that has nothing for the viewer to decide.
    Run(FileAction),
    /// Save the open file in place with this change.
    Edit(EditRequest),
    /// Take back the last edit of the open file, or do it again.
    Rewind(Rewind),
    /// List the kept versions of the open file: the answer is `Sheet(OpenRevert)`.
    ListVersions,
    /// Propose a name for a copy of the open file: the answer is `Sheet(AskSaveCopy)`.
    NameCopy,
    /// Choose another file to open.
    PickFile,
    /// Close the window.
    CloseWindow,
}

/// Everything the regions read besides their inputs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewerParams {
    /// The chrome's timing.
    pub chrome: ChromeParams,
    /// The tabs the open file has.
    pub panel: PanelParams,
    /// The palette's ranked rows for its query.
    pub palette: PaletteParams,
    /// Whether the open file is media.
    pub presentation: PresentationParams,
    /// What the sheets open on.
    pub sheet: SheetParams,
    /// What the stages need from the view and settings.
    pub stage: StageParams,
}
