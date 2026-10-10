//! The one routing function.

use super::model::{Regions, Route};
use crate::chrome::{ChromeIn, PinReason};
use crate::command::StageCommand;
use crate::context::{ContextIn, ContextMenu};
use crate::edits::Rewind;
use crate::hand::{HandIn, Tool};
use crate::io::DesktopService;
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn};
use crate::panel::{Panel, PanelIn, PanelTab};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::Stage;
use crate::typed::TypedText;
use ds_core::standard_action::StandardAction;
use ds_core::vocab::{Shortcut, ShortcutKey};

/// Which region gets `key`, given the states of the regions that can claim it: a sheet, then the
/// palette, then the context menu, then the global chords (⌘K, ⌘I, ⌘W, ⌘O, the Menu key, Esc),
/// then the stage, then navigation, then the chrome.
///
/// Esc undoes the innermost thing: what the stage has open (a find, a scrub), then the
/// panel, then the quick look itself.
pub fn route(key: &Shortcut, regions: Regions<'_>) -> Route {
    let keys = key.keys();
    let keys = keys.as_slice();
    match regions.sheet {
        Sheet::Export { draft: _, span: _ }
        | Sheet::Unavailable {
            needs: _,
            helper: _,
        }
        | Sheet::ConfirmTrash
        | Sheet::ConfirmEdit {
            request: _,
            caution: _,
        }
        | Sheet::Rename { name: _ }
        | Sheet::SaveCopy { name: _ }
        | Sheet::Revert {
            versions: _,
            chosen: _,
        }
        | Sheet::NoVersions
        | Sheet::Helper {
            helper: _,
            phase: _,
        }
        | Sheet::Picture(_)
        | Sheet::Unsaved(_)
        | Sheet::ConfirmReplace => {
            return SheetIn::from_key(keys).map_or(Route::Swallowed, Route::Sheet);
        }
        Sheet::Closed => {}
    }
    match regions.palette {
        Palette::Open {
            query: _,
            selection: _,
            scope: _,
        } => {
            // ⌘F in the open palette makes what is typed a find, where the file can be searched.
            if keys == [ShortcutKey::Super, ShortcutKey::Char('f')] {
                return if can_find(regions.stage) {
                    Route::Palette(PaletteIn::ToFind)
                } else {
                    Route::Swallowed
                };
            }
            return PaletteIn::from_key(keys).map_or(Route::Swallowed, Route::Palette);
        }
        Palette::Closed => {}
    }
    match regions.context {
        ContextMenu::Open { at: _ } => {
            return match keys {
                [ShortcutKey::Escape] => Route::Context(ContextIn::Close),
                _ => Route::Swallowed,
            };
        }
        ContextMenu::Closed => {}
    }
    match global(keys, &regions) {
        Some(route) => route,
        None => unclaimed(keys, &regions),
    }
}

/// The global chords.
fn global(keys: &[ShortcutKey], regions: &Regions<'_>) -> Option<Route> {
    match keys {
        [ShortcutKey::Super, ShortcutKey::Char('k')] => Some(Route::OpenPalette),
        [ShortcutKey::Super, ShortcutKey::Char('f')] if can_find(regions.stage) => {
            Some(Route::OpenFind)
        }
        [ShortcutKey::Super, ShortcutKey::Char('i')] => {
            Some(Route::Panel(info_toggle(regions.panel)))
        }
        [ShortcutKey::Super, ShortcutKey::Char('w')] => Some(Route::CloseWindow),
        [ShortcutKey::Super, ShortcutKey::Char('s')]
            if matches!(regions.stage, Stage::Raster(_)) =>
        {
            Some(Route::Save)
        }
        [ShortcutKey::Super, ShortcutKey::Char('o')]
            if regions.platform.has(DesktopService::FileChooser) =>
        {
            Some(Route::OpenFile)
        }
        [ShortcutKey::ContextMenu] => Some(Route::OpenContextMenu),
        [ShortcutKey::Escape] => Some(escape(regions)),
        keys => rewind(keys).map(Route::Rewind),
    }
}

/// Whether the file on screen can be searched: ⌘F opens the palette as a find only then.
fn can_find(stage: &Stage) -> bool {
    stage.find_input(&TypedText::EMPTY).is_some()
}

/// The standard undo and redo keys.
fn rewind(keys: &[ShortcutKey]) -> Option<Rewind> {
    [
        (StandardAction::Undo, Rewind::Undo),
        (StandardAction::Redo, Rewind::Redo),
    ]
    .into_iter()
    .find(|(standard, _)| Shortcut::standard(*standard).keys() == keys)
    .map(|(_, rewind)| rewind)
}

/// ⌘I: close the panel when it already shows Info, otherwise show Info.
fn info_toggle(panel: &Panel) -> PanelIn {
    match panel {
        Panel::Shown {
            tab: PanelTab::Info,
        } => PanelIn::Close,
        Panel::Hidden
        | Panel::Shown {
            tab: PanelTab::Thumbnails | PanelTab::Contents | PanelTab::Tracks,
        } => PanelIn::Choose(PanelTab::Info),
    }
}

/// H switches the pan tool, C chooses the crop tool, and Space held turns the pan tool on, for a picture. (An animation's Space is
/// its play button, which the stage claimed before this.)
fn hand_key(keys: &[ShortcutKey], stage: &Stage) -> Option<HandIn> {
    if !matches!(stage, Stage::Raster(_)) {
        return None;
    }
    match keys {
        [ShortcutKey::Char('h')] => Some(HandIn::Toggle),
        [ShortcutKey::Char('c')] => Some(HandIn::Use(Tool::Crop)),
        [ShortcutKey::Space] => Some(HandIn::SpaceDown),
        _ => None,
    }
}

fn escape(regions: &Regions<'_>) -> Route {
    if let Some(input) = regions.stage.dismissal() {
        return Route::Stage(input);
    }
    match regions.panel {
        Panel::Shown { tab: _ } => Route::Panel(PanelIn::Close),
        Panel::Hidden => Route::Dismiss,
    }
}

/// A key no global chord wants: the stage if it has the command, then navigation, then the
/// chrome.
fn unclaimed(keys: &[ShortcutKey], regions: &Regions<'_>) -> Route {
    let staged = StageCommand::from_key(keys)
        .and_then(|command| regions.stage.input_for(command, regions.stage_params));
    if let Some(input) = staged {
        return Route::Stage(input);
    }
    if let Some(input) = hand_key(keys, regions.stage) {
        return Route::Hand(input);
    }
    if let Some(input) = NavigateIn::from_key(keys) {
        // A row picked in a table or a tree is the reader's place: Left and Right do not carry
        // them off to another file until Esc puts the cursor away.
        if regions.stage.has_cursor() {
            return Route::Ignored;
        }
        return Route::Navigate(input);
    }
    match keys {
        [ShortcutKey::Tab] => Route::Chrome(ChromeIn::Pin(PinReason::KeyboardFocus)),
        [ShortcutKey::Shift, ShortcutKey::Tab] => {
            Route::Chrome(ChromeIn::Unpin(PinReason::KeyboardFocus))
        }
        _ => Route::Ignored,
    }
}
