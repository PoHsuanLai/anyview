//! Preview | Source, the mode control of a text file with two ways of looking at it: it sits on the
//! titlebar's trailing side, where the window shows a mode that stays (the same place as a
//! picture's Select | Pan).

use crate::TextView;
use dioxus::prelude::*;
use ds::components::controls::segmented::Tracking;
use ds::prelude::{Choice, SegmentedControl, Shortcut, ShortcutKey, Tooltip};

/// The control for a file shown as `view`: picking the other segment asks for that view. The tip is
/// the owner's terse `Name  Key`; the key is V, which switches the view.
#[component]
pub(super) fn ViewModes(view: TextView, onpick: EventHandler<TextView>) -> Element {
    rsx! {
        Tooltip {
            text: "View Mode",
            shortcut: Some(Shortcut(vec![ShortcutKey::Char('v')])),
            div {
                class: "viewer-modes",
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
                SegmentedControl::<TextView> {
                    label: "View mode",
                    choices: vec![
                        Choice::new(TextView::Rendered, "Preview"),
                        Choice::new(TextView::Source, "Source"),
                    ],
                    tracking: Tracking::SelectOne(view),
                    onchange: move |picked: TextView| {
                        if picked != view {
                            onpick.call(picked);
                        }
                    },
                }
            }
        }
    }
}
