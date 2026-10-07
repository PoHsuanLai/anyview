//! The one routing function.

use super::model::{Regions, Route};
use crate::chrome::{ChromeIn, PinReason};
use crate::command::StageCommand;
use crate::context::{ContextIn, ContextMenu};
use crate::edits::Rewind;
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn};
use crate::panel::{Panel, PanelIn, PanelTab};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::Stage;
use ds_core::standard_action::StandardAction;
use ds_core::vocab::{Shortcut, ShortcutKey};

/// Which region gets `key`, given the states of the regions that can claim it: a sheet, then the
/// palette, then the context menu, then the global chords (⌘K, ⌘I, ⌘W, ⌘O, the Menu key, Esc),
/// then the stage, then navigation, then the chrome.
///
/// Esc undoes the innermost thing: what the stage has open (a find bar, a scrub), then the
/// panel, then the quick look itself.
pub fn route(key: &Shortcut, regions: Regions<'_>) -> Route {
    let keys = key.keys();
    let keys = keys.as_slice();
    match regions.sheet {
        Sheet::Export { draft: _ }
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
        } => {
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
        [ShortcutKey::Super, ShortcutKey::Char('i')] => {
            Some(Route::Panel(info_toggle(regions.panel)))
        }
        [ShortcutKey::Super, ShortcutKey::Char('w')] => Some(Route::CloseWindow),
        [ShortcutKey::Super, ShortcutKey::Char('o')] if regions.pick_files => Some(Route::OpenFile),
        [ShortcutKey::ContextMenu] => Some(Route::OpenContextMenu),
        [ShortcutKey::Escape] => Some(escape(regions)),
        keys => rewind(keys, regions.stage).map(Route::Rewind),
    }
}

/// The standard undo and redo keys, unless a find bar is up: its field has its own undo.
fn rewind(keys: &[ShortcutKey], stage: &Stage) -> Option<Rewind> {
    if stage.is_finding() {
        return None;
    }
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
