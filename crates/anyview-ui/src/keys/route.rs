//! The one routing function.

use super::model::{Regions, Route};
use crate::chrome::{ChromeIn, PinReason};
use crate::command::StageCommand;
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn};
use crate::panel::{Panel, PanelIn, PanelTab};
use crate::sheet::{Sheet, SheetIn};
use ds_core::vocab::{Shortcut, ShortcutKey};

/// Which region gets `key`, given the states of the regions that can claim it: a sheet, then the
/// palette, then the global chords (⌘K, ⌘I, ⌘W, ⌘O, Esc), then the stage, then navigation, then
/// the chrome.
///
/// Esc undoes the innermost thing: what the stage has open (a find bar, a scrub), then the
/// panel, then the quick look itself.
pub fn route(key: &Shortcut, regions: Regions<'_>) -> Route {
    let keys = key.keys();
    let keys = keys.as_slice();
    match regions.sheet {
        Sheet::Export { draft: _ }
        | Sheet::Unavailable { needs: _ }
        | Sheet::ConfirmTrash
        | Sheet::Rename { name: _ } => {
            return SheetIn::from_key(keys).map_or(Route::Swallowed, Route::Sheet);
        }
        Sheet::Closed => {}
    }
    match regions.palette {
        Palette::Open {
            query: _,
            selection: _,
        } => return PaletteIn::from_key(keys).map_or(Route::Swallowed, Route::Palette),
        Palette::Closed => {}
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
        [ShortcutKey::Super, ShortcutKey::Char('i')] => {
            Some(Route::Panel(info_toggle(regions.panel)))
        }
        [ShortcutKey::Super, ShortcutKey::Char('w')] => Some(Route::CloseWindow),
        [ShortcutKey::Super, ShortcutKey::Char('o')] => Some(Route::OpenFile),
        [ShortcutKey::Escape] => Some(escape(regions)),
        _ => None,
    }
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
    if let Some(input) = NavigateIn::from_key(keys) {
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
