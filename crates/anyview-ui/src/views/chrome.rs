//! The chrome that comes and goes: the titlebar across the top and the capsule at the bottom, both
//! shown while the chrome machine says so. The machine decides when (the pointer moved, idle for
//! `hide_after`, a menu open); the views carry the fade out with `ds-motion`'s presence and tell
//! the machine where the pointer is.

use crate::Command;
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::view::Capsule;
use ds::components::chrome::titlebar_parts::TitleParts;
use ds::components::chrome::window_frame::{TrafficLights, WindowTitlebar};
use ds::prelude::{Material, Shown, Surface};
use ds_core::word::Word;

/// The titlebar: the window's title and lights on the bar material, over the top of the content.
/// It takes no pointer while it is hidden, so it never blocks the content under it.
#[component]
pub(super) fn Titlebar(
    title: String,
    shown: Shown,
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
                    parts: TitleParts::default(),
                    lights: TrafficLights::Shown,
                }
            }
        }
    }
}

/// The capsule of the open file's controls.
#[component]
pub(super) fn Controls(
    slots: Vec<CapsuleSlot<Command>>,
    shown: Shown,
    onpick: EventHandler<Command>,
    onpointerenter: EventHandler<()>,
    onpointerleave: EventHandler<()>,
) -> Element {
    rsx! {
        Capsule::<Command> {
            label: "Controls",
            slots,
            shown,
            onpick,
            onpointerenter,
            onpointerleave,
        }
    }
}
