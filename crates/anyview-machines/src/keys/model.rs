//! The regions a key can go to, and the states that decide it.

use super::act::Act;
use crate::PlatformAbilities;
use crate::chrome::ChromeIn;
use crate::context::{ContextIn, ContextMenu};
use crate::edits::Rewind;
use crate::hand::HandIn;
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn};
use crate::panel::{Panel, PanelIn};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::{Stage, StageIn, StageParams};
use ds_core::vocab::{Shortcut, ShortcutKey};

/// A key press as the viewer reads it: a chord the keymap resolved to one of the viewer's actions,
/// or a key with no command modifier (typing, an arrow, Esc, Enter, Space, Shift and an arrow).
/// Which modifier is Command or Ctrl is the keymap's to say, so a press is never both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Press {
    /// A key as it was pressed.
    Key(Shortcut),
    /// The action the keymap made of a chord.
    Act(Act),
}

impl Press {
    /// The keys of a plain press; none for an action.
    #[must_use]
    pub fn keys(&self) -> Vec<ShortcutKey> {
        match self {
            Press::Key(shortcut) => shortcut.keys(),
            Press::Act(_) => Vec::new(),
        }
    }

    /// The action of a chord; none for a plain key.
    #[must_use]
    pub fn act(&self) -> Option<Act> {
        match self {
            Press::Key(_) => None,
            Press::Act(act) => Some(*act),
        }
    }

    /// Whether a text being edited lets this press through to the window: Esc and the actions
    /// that are not the text's own (undo, redo, the clipboard, moving and deleting stay in it).
    #[must_use]
    pub fn for_the_window(&self) -> bool {
        match self {
            Press::Key(shortcut) => shortcut.keys() == [ShortcutKey::Escape],
            Press::Act(act) => !act.belongs_to_text(),
        }
    }
}

/// Whether the viewer answers the chords of the keymap (a Command or Ctrl key with a letter).
/// A pane is hosted: the host owns every one of them, and what the viewer answers there is the
/// plain keys alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Chords {
    /// The viewer answers its chords: a window.
    #[default]
    Viewer,
    /// The viewer answers none; a chord goes nowhere.
    None,
}

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
    /// The desktop services there are: Open is a chord only where there is a file chooser.
    pub platform: PlatformAbilities,
    /// Whether the viewer's chords are answered.
    pub chords: Chords,
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
    /// The palette's action (⌘K): open it.
    OpenPalette,
    /// Find (⌘F): open the palette as a find in the open file.
    OpenFind,
    /// Undo and Redo (⌘Z, ⇧⌘Z): take back the last edit, or do it again.
    Rewind(Rewind),
    /// The Info action opens or closes the Info tab; Esc closes the panel.
    Panel(PanelIn),
    /// Close (⌘W): close the window.
    CloseWindow,
    /// Save (⌘S) on a picture: write the changes made to it into the file.
    Save,
    /// Open (⌘O): choose another file to open.
    OpenFile,
    /// Esc with nothing open to close: the viewer decides what that means for how it is shown
    /// (a quick look closes).
    Dismiss,
    /// The stage claimed the key, including Esc closing what the stage has open.
    Stage(StageIn),
    /// H, or Space held on a still picture: the pan tool.
    Hand(HandIn),
    /// ← → Home End walk the sequence.
    Navigate(NavigateIn),
    /// Tab and ⇧Tab move keyboard focus into and out of the chrome.
    Chrome(ChromeIn),
    /// No region wants the key.
    Ignored,
}
