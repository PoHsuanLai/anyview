//! The mode control: the pointer tools a picture offers.

use crate::{Command, Tool};
use dioxus::prelude::*;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::{Button, Check, Icon, IconButtonGroup, IconSource};
use ds::style::tokens::control_size::ControlSize;

/// Pan, and Crop beside it when the picture can be saved with changes (Markup on a PDF that can
/// be marked), as toggle buttons in one group like Preview's toolbar: Select is the state with none
/// pressed, and pressing the pressed one goes back to it. A drag pans a zoomed picture under Pan (or
/// while Space is held) and moves the corners of the crop rectangle under Crop. The group sits on
/// the titlebar's trailing side, where the window can show a mode that stays. Each tip is the
/// owner's terse `Name  Key`: H switches Pan, C Crop.
#[component]
pub(crate) fn PointerModes(tool: Tool, tools: Vec<Tool>, onpick: EventHandler<Tool>) -> Element {
    rsx! {
        div {
            class: "viewer-modes",
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
            IconButtonGroup { label: "Pointer mode",
                for offered in tools.iter().copied().filter(|offered| *offered != Tool::Select) {
                    Button {
                        bezel: Bezel::Toolbar,
                        image: ImagePosition::Only,
                        size: ControlSize::Small,
                        label: offered.label(),
                        icon: Some(IconSource::from(glyph(offered))),
                        value: Some(if tool == offered { Check::On } else { Check::Off }),
                        title: Some(offered.label().to_owned()),
                        title_shortcut: Command::UseTool(offered).shortcut(),
                        onclick: move |_| {
                            onpick.call(if tool == offered { Tool::Select } else { offered });
                        },
                    }
                }
            }
        }
    }
}

/// The glyph that stands for `tool`; Select is the state with nothing pressed, so it has none of
/// its own.
fn glyph(tool: Tool) -> Icon {
    match tool {
        Tool::Select | Tool::Pan => Icon::Hand,
        Tool::Crop => Icon::Crop,
        Tool::Markup => Icon::Pen,
    }
}
