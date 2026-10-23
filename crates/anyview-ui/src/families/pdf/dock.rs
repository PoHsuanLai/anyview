//! The Markup tool's bar: what the pointer draws, and the colour of it. It sits above the capsule
//! while the tool is on, as the crop bar does over a picture.

use crate::{Command, MarkupCommand};
use anyview_core::{MarkColor, MarkTool};
use dioxus::prelude::*;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::{Button, Check, Icon, IconButtonGroup, IconSource};
use ds::style::tokens::control_size::ControlSize;
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

/// The glyph that stands for `tool` on its button.
fn glyph(tool: MarkTool) -> Icon {
    match tool {
        MarkTool::Pointer => Icon::MousePointer,
        MarkTool::Highlight => Icon::Highlighter,
        MarkTool::Underline => Icon::Underline,
        MarkTool::Strikethrough => Icon::Strike,
        MarkTool::Note => Icon::StickyNote,
        MarkTool::TextBox => Icon::Type,
        MarkTool::Rectangle => Icon::Square,
        MarkTool::Oval => Icon::Circle,
        MarkTool::Line => Icon::Line,
        MarkTool::Arrow => Icon::ArrowUpRight,
        MarkTool::Sketch => Icon::PenLine,
    }
}

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
                    IconButtonGroup { label: "Mark",
                        for mark in group.iter().copied() {
                            Button {
                                bezel: Bezel::Toolbar,
                                image: ImagePosition::Only,
                                size: ControlSize::Small,
                                label: mark.label(),
                                icon: Some(IconSource::from(glyph(mark))),
                                value: Some(if mark == tool { Check::On } else { Check::Off }),
                                title: Some(mark.label().to_owned()),
                                title_shortcut: Command::Markup(MarkupCommand::Tool(mark)).shortcut(),
                                onclick: move |_| {
                                    run.call(Command::Markup(MarkupCommand::Tool(mark)));
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
                        bezel: Bezel::Toolbar,
                        image: ImagePosition::Only,
                        size: ControlSize::Small,
                        label: "Delete",
                        icon: Some(IconSource::from(Icon::Trash)),
                        title: Some("Delete".to_owned()),
                        title_shortcut: Command::Markup(MarkupCommand::Delete).shortcut(),
                        onclick: move |_| run.call(Command::Markup(MarkupCommand::Delete)),
                    }
                }
            }
        }
    }
}
