//! The root's state, inputs, outputs and parameters.

use crate::chrome::{Chrome, ChromeIn, ChromeOut, ChromeParams};
use crate::command::Command;
use crate::context::{ContextIn, ContextMenu, ContextParams};
use crate::edits::{EditRequest, Rewind};
use crate::hand::{Hand, HandIn};
use crate::keys::Press;
use crate::load::{Load, LoadIn, LoadOut, Ticket};
use crate::navigate::{Navigate, NavigateIn};
use crate::palette::{Palette, PaletteIn, PaletteOut, PaletteParams};
use crate::panel::{Panel, PanelIn, PanelOut, PanelParams};
use crate::picture::{PictureEditIn, PictureEdits};
use crate::presentation::{Presentation, PresentationIn, PresentationOut, PresentationParams};
use crate::sheet::{Departure, Sheet, SheetIn, SheetOut, SheetParams};
use crate::stage::{Changes, Stage, StageIn, StageOut, StageParams};
use crate::{PlatformAbilities, SaveEnd};
use anyview_core::{FileAction, FilePath};

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
    /// The right-click menu.
    pub context: ContextMenu,
    /// The modal sheet.
    pub sheet: Sheet,
    /// Walking the sequence.
    pub navigate: Navigate,
    /// How the viewer is on screen.
    pub presentation: Presentation,
    /// What shows the content.
    pub stage: Stage,
    /// Whether a drag pans a picture.
    pub hand: Hand,
    /// Whether the person has sent the open file to the Trash, so that its going is expected.
    pub trashing: Trashing,
    /// Whether a file chooser has been asked for and has not answered.
    pub choosing: Choosing,
    /// Whether the person has opened or closed the side panel in this window.
    pub panel_said: PanelSay,
    /// What has been done to the open picture and not saved.
    pub picture: PictureEdits,
    /// Where the person was going when they chose to save the picture's changes first: they go
    /// when the saved file has been read again.
    pub after_save: Option<Departure>,
}

/// Whether the person has said what they want of the side panel. Until they have, a PDF or a
/// book opens it on its pages, as Preview does; once they have, the panel stays as they left it
/// from file to file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelSay {
    /// The panel is as the viewer left it: no choice yet.
    #[default]
    Unsaid,
    /// The person opened, closed or switched the panel.
    Said,
}

/// Whether the window is waiting on a file chooser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Choosing {
    /// No chooser is up.
    #[default]
    Not,
    /// A chooser was asked for; asking again would open a second one on top of it.
    Asked,
}

/// Whether the open file is on its way to the Trash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trashing {
    /// The file is not being trashed.
    #[default]
    Not,
    /// The person confirmed Move to Trash; the file's disappearance is the answer to that, not a
    /// surprise.
    Underway,
}

impl Viewer {
    /// Whether going away from the file shown would lose changes that are not saved: a picture's
    /// edits, or a text's.
    pub fn unsaved(&self) -> bool {
        let picture = self.picture.is_edited() && matches!(self.stage, Stage::Raster(_));
        let text = self
            .stage
            .edited()
            .is_some_and(|edited| edited.changes == Changes::Unsaved);
        picture || text
    }

    /// A viewer launched in `presentation` (a quick look, a window, a background session): it is
    /// so from the start, and nothing is asked of the host (it made the window that way). The
    /// window seeds the machine with it.
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
    /// The file chooser ended with these files, none when it was cancelled or could not open. A
    /// list is opened as a drop is.
    Chosen(Vec<FilePath>),
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
    /// The context menu.
    Context(ContextIn),
    /// The sheet.
    Sheet(SheetIn),
    /// Walking the sequence.
    Navigate(NavigateIn),
    /// How the viewer is on screen.
    Presentation(PresentationIn),
    /// The stage showing the file.
    Stage(StageIn),
    /// The pan tool, or Space held for it.
    Hand(HandIn),
    /// An edit of the open picture, or the crop rectangle being moved.
    Picture(PictureEditIn),
    /// Run a command from a control the window drew (a capsule button): the same thing the
    /// palette runs for the row it picked.
    Run(Command),
    /// A key press, routed by `route`.
    Key(Press),
    /// The host's save of the edited text ended like this.
    Saved(SaveEnd),
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
    /// Ask for a new name for the open file: the answer is `Sheet(AskRename)`, starting from the
    /// current name.
    NameRename,
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
    /// The rows of the context menu.
    pub context: ContextParams,
    /// Whether the open file is media.
    pub presentation: PresentationParams,
    /// What the sheets open on.
    pub sheet: SheetParams,
    /// What the stages need from the view and settings.
    pub stage: StageParams,
    /// The file actions the open file offers, whatever the palette's query: the ones a key may
    /// run.
    pub files: Vec<FileAction>,
    /// What the platform can do: Open is bound only when it has a file chooser.
    pub platform: PlatformAbilities,
}
