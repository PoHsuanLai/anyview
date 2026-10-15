//! The Markup tool's bar: what the pointer draws, and the colour of it. It sits above the capsule
//! while the tool is on, as the crop bar does over a picture.

use crate::{Command, MarkupCommand};
use anyview_core::{MarkColor, MarkTool};
use dioxus::prelude::*;
use ds::components::controls::button::Button;
use ds::components::controls::segmented::Tracking;
use ds::prelude::{Choice, SegmentedControl};
use ds_core::word::Word;

/// The tools in the three groups the bar shows them in.
const GROUPS: [&[MarkTool]; 3] = [
    &[
        MarkTool::Pointer,
        MarkTool::Highlight,
        MarkTool::Underline,
        MarkTool::Strikethrough,
    ],
    &[MarkTool::Note, MarkTool::TextBox],
    &[
        MarkTool::Rectangle,
        MarkTool::Oval,
        MarkTool::Line,
        MarkTool::Arrow,
        MarkTool::Sketch,
    ],
];

/// The bar. `picked` is whether a mark is picked, which Delete takes away.
#[component]
pub(super) fn MarkupDock(
    tool: MarkTool,
    color: MarkColor,
    picked: bool,
    run: EventHandler<Command>,
) -> Element {
    rsx! {
        div { class: "viewer-markup-dock",
            div {
                class: "viewer-markup-bar",
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
                for group in GROUPS {
                    {
                        let choices: Vec<Choice<MarkTool>> = group
                            .iter()
                            .map(|tool| Choice::new(*tool, tool.label()))
                            .collect();
                        rsx! {
                            SegmentedControl::<MarkTool> {
                                label: "Mark",
                                choices,
                                tracking: Tracking::SelectOne(tool),
                                onchange: move |chosen: MarkTool| {
                                    run.call(Command::Markup(MarkupCommand::Tool(chosen)));
                                },
                            }
                        }
                    }
                }
                div { class: "viewer-swatches", role: "radiogroup", "aria-label": "Colour",
                    for swatch in MarkColor::ALL.iter().copied() {
                        {
                            let (red, green, blue) = swatch.rgb();
                            let paint = format!("--mark:rgb({red}, {green}, {blue})");
                            rsx! {
                                button {
                                    r#type: "button",
                                    class: "viewer-swatch",
                                    role: "radio",
                                    "aria-label": swatch.label(),
                                    "aria-checked": if swatch == color { "true" } else { "false" },
                                    "data-on": if swatch == color { "on" } else { "off" },
                                    style: paint,
                                    onclick: move |_| {
                                        run.call(Command::Markup(MarkupCommand::Color(swatch)));
                                    },
                                }
                            }
                        }
                    }
                }
                if picked {
                    Button {
                        label: "Delete",
                        onclick: move |_| run.call(Command::Markup(MarkupCommand::Delete)),
                    }
                }
            }
        }
    }
}
