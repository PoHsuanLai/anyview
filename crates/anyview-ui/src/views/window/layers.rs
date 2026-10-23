//! The layers over the viewer's content: the titlebar, the palette and the context menu, each
//! drawn from the machine's state and answering with inputs.

use super::super::chrome::{BarRoom, Titlebar};
use super::super::context::ContextPopup;
use super::super::palette::Palette;
use super::super::press::press_of;
use super::super::shelf::Dispatch;
use crate::{
    Chords, Command, ContextEntry, ContextIn, ContextMenu, HitLine, Palette as PaletteState,
    PaletteIn, Presentation, TypedText, Viewer, ViewerIn, Zone,
};
use chordkit::Context;
use dioxus::prelude::*;
use ds::components::chrome::titlebar_parts::DocumentState;
use ds::prelude::*;

/// The titlebar, over the panel's pane and the stage alike. The small borderless window and a pane
/// have none: the picture is what is dragged. The pointer over the bar is the capsule's zone.
pub(super) fn titlebar_view(
    presentation: Presentation,
    title: String,
    room: BarRoom,
    shown: Shown,
    leading: Option<Element>,
    trailing: Option<Element>,
    document: DocumentState,
    mut zone: Signal<Zone>,
) -> Element {
    match presentation {
        Presentation::Mini | Presentation::Pane => rsx! {},
        Presentation::Window | Presentation::Peek | Presentation::Background => rsx! {
            Titlebar {
                title,
                room,
                shown,
                leading,
                trailing,
                document,
                onpointerenter: move |()| zone.set(Zone::Capsule),
                onpointerleave: move |()| zone.set(Zone::Content),
            }
        },
    }
}

/// The palette, while it is open: its `rows`, the `hits` and the count `found` of a find, and a
/// key typed in it, which goes through the `keys` (the keymap and whose chords are bound).
pub(super) fn palette_view(
    state: &Viewer,
    rows: Vec<Command>,
    hits: Vec<HitLine>,
    found: u32,
    keys: (ds::prelude::Keys, Chords),
    dispatch: Dispatch,
) -> Element {
    let (keymap, chords) = keys;
    let PaletteState::Open {
        query: typed,
        selection,
        ..
    } = &state.palette
    else {
        return rsx! {};
    };
    rsx! {
        Palette {
            query: typed.clone(),
            rows,
            selection: *selection,
            scope: state.palette_scope,
            hits,
            found,
            options: state.find_options,
            ontyped: move |text: TypedText| dispatch.send(ViewerIn::Palette(PaletteIn::Typed(text))),
            onpick: move |row| dispatch.send(ViewerIn::Palette(PaletteIn::Pick(row))),
            onkey: move |event: KeyboardEvent| {
                if let Some(press) = press_of(keymap, &event, Context::TextEntry, chords) {
                    dispatch.send(ViewerIn::Key(press));
                }
            },
            onclose: move |()| dispatch.send(ViewerIn::Palette(PaletteIn::Close)),
        }
    }
}

/// The context menu, while it is open, listing `entries` at the spot it opened at.
pub(super) fn context_view(
    context: &ContextMenu,
    entries: Vec<ContextEntry>,
    dispatch: Dispatch,
) -> Element {
    let ContextMenu::Open { at } = context else {
        return rsx! {};
    };
    rsx! {
        ContextPopup {
            entries,
            at: *at,
            onpick: move |pick| dispatch.send(ViewerIn::Context(ContextIn::Pick(pick))),
            onclose: move |()| dispatch.send(ViewerIn::Context(ContextIn::Close)),
        }
    }
}
