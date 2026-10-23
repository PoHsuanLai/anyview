//! The chrome that comes and goes: the titlebar across the top and the capsule at the bottom, both
//! shown while the chrome machine says so. The machine decides when (the pointer moved, idle for
//! `hide_after`, a menu open); the views carry the fade out with `ds-motion`'s presence and tell
//! the machine where the pointer is.

use crate::Command;
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::ScrubEvent;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::components::chrome::capsule::view::Capsule;
use ds::components::chrome::titlebar_parts::{DocumentState, TitleParts};
use ds::components::chrome::window_frame::{TitleRoom, TrafficLights, WindowTitlebar};
use ds::prelude::{Material, Shown, Surface};
use ds_core::vocab::Fraction;
use ds_core::word::Word;

/// How much of the titlebar the family's mode control takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum BarRoom {
    /// The title is centred and the control sits right of it.
    #[default]
    Roomy,
    /// The window is narrow: the title ends before the control.
    Tight,
    /// The window has no room for the control; it is not drawn.
    Bare,
}

impl BarRoom {
    /// Where the title sits: beside the mode control when it takes the right of the bar, else
    /// centred.
    fn title_room(self) -> TitleRoom {
        match self {
            BarRoom::Tight => TitleRoom::Beside,
            BarRoom::Roomy | BarRoom::Bare => TitleRoom::Centred,
        }
    }
}

/// The titlebar: the window's title and lights on the bar material, over the top of the content.
/// It takes no pointer while it is hidden, so it never blocks the content under it.
#[component]
pub(super) fn Titlebar(
    title: String,
    #[props(default)] edited: DocumentState,
    #[props(default)] room: BarRoom,
    shown: Shown,
    leading: Option<Element>,
    trailing: Option<Element>,
    #[props(default)] document: DocumentState,
    onpointerenter: EventHandler<()>,
    onpointerleave: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "viewer-titlebar",
            "data-shown": shown.slug(),
            onpointerenter: move |_| onpointerenter.call(()),
            onpointerleave: move |_| onpointerleave.call(()),
            Surface { material: Material::Bar,
                WindowTitlebar {
                    title,
                    parts: TitleParts {
                        document,
                        ..TitleParts::default()
                    },
                    lights: TrafficLights::Shown,
                    title_room: room.title_room(),
                    // The sidebar's button, after the lights.
                    leading,
                    // The family's mode controls (Select | Pan) are part of the bar.
                    trailing,
                }
            }
        }
    }
}

/// The capsule of the open file's controls.
#[component]
pub(super) fn Controls(
    slots: Vec<RankedSlot<Command>>,
    shown: Shown,
    onpick: EventHandler<Command>,
    onscrub: EventHandler<ScrubEvent>,
    onlevel: EventHandler<Fraction>,
    onpointerenter: EventHandler<()>,
    onpointerleave: EventHandler<()>,
) -> Element {
    rsx! {
        Capsule::<Command> {
            label: "Controls",
            slots,
            shown,
            onpick,
            onscrub,
            onlevel,
            onpointerenter,
            onpointerleave,
        }
    }
}
