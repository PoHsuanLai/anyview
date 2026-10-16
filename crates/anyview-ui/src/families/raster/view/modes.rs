//! The mode control: the pointer tools a picture offers.

use crate::Tool;
use dioxus::prelude::*;
use ds::components::controls::segmented::Tracking;
use ds::prelude::{Choice, SegmentedControl, Shortcut, ShortcutKey, Tooltip};

/// Select | Pan, as Preview's tool control, and Crop beside them when the picture can be saved with
/// changes: a drag pans a zoomed picture under Pan (or while Space is held) and moves the corners of
/// the crop rectangle under Crop. It sits on the titlebar's trailing side, where the window can show
/// a mode that stays. The tip is the owner's terse `Name  Key`: the key is H, which switches the
/// tool.
#[component]
pub(crate) fn PointerModes(tool: Tool, tools: Vec<Tool>, onpick: EventHandler<Tool>) -> Element {
    let choices: Vec<Choice<Tool>> = tools
        .iter()
        .map(|offered| Choice::new(*offered, offered.label()))
        .collect();
    rsx! {
        Tooltip {
            text: "Pointer Tool",
            shortcut: Some(Shortcut(vec![ShortcutKey::Char('h')])),
            div {
                class: "viewer-modes",
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
                SegmentedControl::<Tool> {
                    label: "Pointer mode",
                    choices,
                    tracking: Tracking::SelectOne(tool),
                    onchange: move |picked: Tool| {
                        if picked != tool {
                            onpick.call(picked);
                        }
                    },
                }
            }
        }
    }
}
