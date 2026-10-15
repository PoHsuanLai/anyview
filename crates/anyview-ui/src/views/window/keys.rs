//! The window's keys: which key presses reach the machine, given what holds the keyboard (a pane
//! the host has not given it to, the palette, a menu, a sheet, text being edited).

use super::super::press::{CHORD_KEYS, press_of};
use super::super::shelf::Dispatch;
use crate::{Chords, SheetIn, StageIn, TypedText, ViewerIn};
use chordkit::Context;
use dioxus::prelude::*;
use ds::prelude::Keys;

/// What holds the keyboard as a key goes down.
#[derive(Clone, Copy)]
pub(super) struct KeyScene {
    /// The palette is up and takes its own keys.
    pub palette_open: bool,
    /// The context menu is up and takes its own keys.
    pub context_open: bool,
    /// Text being edited, or a form field, takes the keys the window does not need.
    pub editing: bool,
    /// A sheet is up and answers Return and Esc.
    pub sheet_open: bool,
    /// The window's keymap.
    pub keymap: Keys,
    /// Whose chords are bound: the viewer's, or the host's.
    pub chords: Chords,
}

/// Send the machine what the key `event` means, given `scene`: nothing while the palette or a menu
/// has the keys, only the window's own while text is edited, and a sheet's answers while a sheet
/// is up.
pub(super) fn window_key(event: &KeyboardEvent, scene: KeyScene, dispatch: Dispatch) {
    // The palette and the context menu take their own keys. A sheet does too, but when
    // focus is still on the window Return and Esc reach it here, so the machine's sheet
    // answers them.
    if scene.palette_open || scene.context_open {
        return;
    }
    // The text being edited has the keys: the window hears only what is meant for it.
    let context = if scene.editing {
        Context::TextEntry
    } else {
        Context::Normal
    };
    let Some(press) = press_of(scene.keymap, event, context, scene.chords) else {
        return;
    };
    if scene.editing && !scene.sheet_open && !press.for_the_window() {
        return;
    }
    if scene.sheet_open {
        if SheetIn::from_press(&press).is_some() {
            event.prevent_default();
            dispatch.send(ViewerIn::Key(press));
        }
        return;
    }
    // A key typed on a picked cell begins typing into it, with that character.
    if let Some(input) = typed_start(event, &dispatch) {
        event.prevent_default();
        dispatch.send(ViewerIn::Stage(input));
        return;
    }
    event.prevent_default();
    dispatch.send(ViewerIn::Key(press));
}

/// The input a key typed now makes of itself, when it is a character typed on a picked cell of an
/// editable table: the cell is typed into, starting with the character.
fn typed_start(event: &KeyboardEvent, dispatch: &Dispatch) -> Option<StageIn> {
    use dioxus::html::ModifiersInteraction as _;
    let Key::Character(text) = event.key() else {
        return None;
    };
    if event.modifiers().intersects(CHORD_KEYS) {
        return None;
    }
    let stage = dispatch.machine.state().peek().stage.clone();
    if !matches!(stage, crate::Stage::Table(_)) {
        return None;
    }
    stage.typed_into(TypedText::new(text), &dispatch.params().stage)
}
