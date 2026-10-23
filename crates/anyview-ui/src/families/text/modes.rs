//! The Source toggle, the mode control of a text file with two ways of looking at it: it sits on
//! the titlebar's trailing side, where the window shows a mode that stays (the same place as a
//! picture's Pan).

use crate::{Command, TextView};
use dioxus::prelude::*;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::{Button, Check, Icon, IconSource};
use ds::style::tokens::control_size::ControlSize;

/// The control for a file shown as `view`: pressed while the source shows, and a press asks for
/// the other view. Its tip names what a press does, then the key; the key is V, which switches
/// the view.
#[component]
pub(super) fn ViewModes(view: TextView, onpick: EventHandler<TextView>) -> Element {
    let (name, other) = match view {
        TextView::Rendered => ("Show Source", TextView::Source),
        TextView::Source => ("Show Preview", TextView::Rendered),
    };
    rsx! {
        div {
            class: "viewer-modes",
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
            Button {
                bezel: Bezel::Toolbar,
                image: ImagePosition::Only,
                size: ControlSize::Small,
                label: name,
                icon: Some(IconSource::from(Icon::Code)),
                value: Some(if view == TextView::Source { Check::On } else { Check::Off }),
                title: Some(name.to_owned()),
                title_shortcut: Command::ShowView(other).shortcut(),
                onclick: move |_| onpick.call(other),
            }
        }
    }
}
