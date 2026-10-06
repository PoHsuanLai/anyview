//! The regions a key can go to, and the states that decide it.

use crate::chrome::ChromeIn;
use crate::context::{ContextIn, ContextMenu};
use crate::edits::Rewind;
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn};
use crate::panel::{Panel, PanelIn};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::{Stage, StageIn, StageParams};

/// The states key routing reads. It decides from these and nothing else.
#[derive(Debug, Clone, Copy)]
pub struct Regions<'a> {
    /// The modal sheet.
    pub sheet: &'a Sheet,
    /// The ⌘K palette.
    pub palette: &'a Palette,
    /// The right-click menu.
    pub context: &'a ContextMenu,
    /// The side panel.
    pub panel: &'a Panel,
    /// The stage that is showing.
    pub stage: &'a Stage,
    /// What the stage needs to turn a command into an input.
    pub stage_params: &'a StageParams,
}

/// Where a key goes, with the input the region is to be given. Precedence is the order of the
/// variants below: a sheet, then the palette, then the context menu, then the global chords, then
/// the stage, then navigation, then the chrome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// A sheet is open and takes the key.
    Sheet(SheetIn),
    /// The palette is open and takes the key.
    Palette(PaletteIn),
    /// The context menu is open and takes the key: Esc closes it. Its other keys (arrows, Enter,
    /// letters) are the menu component's own.
    Context(ContextIn),
    /// A sheet, the palette or the context menu is open and the key means nothing to it; it goes
    /// nowhere, not to the content behind. A palette's field receives typing by itself.
    Swallowed,
    /// The Menu key or ⇧F10: open the context menu at the middle of the content.
    OpenContextMenu,
    /// ⌘K: open the palette.
    OpenPalette,
    /// ⌘Z and ⇧⌘Z: take back the last edit, or do it again.
    Rewind(Rewind),
    /// ⌘I opens or closes the Info tab; Esc closes the panel.
    Panel(PanelIn),
    /// ⌘W: close the window.
    CloseWindow,
    /// ⌘O: choose another file to open.
    OpenFile,
    /// Esc with nothing open to close: the viewer decides what that means for how it is shown
    /// (a quick look closes).
    Dismiss,
    /// The stage claimed the key, including Esc closing what the stage has open.
    Stage(StageIn),
    /// ← → Home End walk the sequence.
    Navigate(NavigateIn),
    /// Tab and ⇧Tab move keyboard focus into and out of the chrome.
    Chrome(ChromeIn),
    /// No region wants the key.
    Ignored,
}
